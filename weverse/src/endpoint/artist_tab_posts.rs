use std::collections::VecDeque;

use anyhow::Result;
use futures::Stream;
use reqwest::{header, Client};
use serde::Deserialize;
use time::OffsetDateTime;

use crate::auth::{compute_url, get_secret};

use super::community_id::CommunityId;
use super::{APP_ID, REFERER};
use crate::utils::deserialize_timestamp;

#[derive(Debug)]
enum PageState {
    Inital,
    Next(String),
    Done,
}

#[derive(Debug)]
pub struct ArtistPosts {
    all_ids: VecDeque<ArtistPostShort>,
    community_id: CommunityId,
    auth: String,
    // For pagination
    page_state: PageState,
}

impl ArtistPosts {
    pub(crate) fn init(community_id: CommunityId, auth: String) -> Self {
        Self {
            all_ids: VecDeque::new(),
            community_id,
            auth,
            page_state: PageState::Inital,
        }
    }

    pub async fn as_stream<'a>(
        &'a mut self,
        client: &'a Client,
    ) -> impl Stream<Item = Result<ArtistPostShort>> + 'a {
        futures::stream::unfold(self, |state| async {
            // Pop off and return the next post if it exists
            if let Some(post_id) = state.all_ids.pop_front() {
                return Some((Ok(post_id), state));
            }

            match &state.page_state {
                // Reached last page
                PageState::Done => None,

                // Fetch next page
                _ => {
                    if let Err(e) = state.fetch_next_page(client).await {
                        return Some((Err(e), state));
                    }
                    state
                        .all_ids
                        .pop_front()
                        .map(|post_id| (Ok(post_id), state))
                }
            }
        })
    }

    async fn fetch_next_page(&mut self, client: &Client) -> Result<()> {
        let secret = get_secret(client).await?;

        let after = match &self.page_state {
            PageState::Inital => "".to_string(),
            PageState::Next(a) => format!("after={}&", a),
            _ => unreachable!(),
        };

        let url = compute_url(
            &format!(
                "/post/v1.0/community-{}/artistTabPosts?{}fieldSet=postsV1&limit=20&pagingType=CURSOR&appId={}&language=en&platform=WEB&wpf=pc",
                self.community_id.id(), after, APP_ID
                ),
                &secret,
                )
            .await?;

        let post_page = client
            .get(url.as_str())
            .header(header::REFERER, REFERER)
            .header(header::AUTHORIZATION, &self.auth)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json::<ArtistPostsResponse>()
            .await
            .unwrap();

        // Update page state
        self.page_state = match post_page.paging {
            Some(paging) => PageState::Next(paging.next_params.after),
            None => PageState::Done,
        };

        // Fill artist posts
        self.all_ids.extend(post_page.data.into_iter());

        Ok(())
    }
}

#[derive(Deserialize, Debug)]
struct ArtistPostsResponse {
    paging: Option<Paging>,
    data: Vec<ArtistPostShort>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Paging {
    next_params: NextParams,
}

#[derive(Deserialize, Debug)]
struct NextParams {
    after: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ArtistPostShort {
    #[serde(rename = "publishedAt")]
    #[serde(deserialize_with = "deserialize_timestamp")]
    pub time: OffsetDateTime,
    pub post_id: String,
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::utils::{setup, LOGIN_INFO};
    use futures::stream::StreamExt;

    #[tokio::test]
    async fn paging() {
        let client = Client::new();
        let auth = LOGIN_INFO.get_or_init(setup()).await;
        let mut artist_posts = ArtistPosts::init(CommunityId::new(14), auth.clone());
        let post_stream = artist_posts.as_stream(&client).await;
        assert_eq!(30, post_stream.take(30).count().await);
    }
}
