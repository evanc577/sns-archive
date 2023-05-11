use std::collections::VecDeque;

use anyhow::Result;
use futures::Stream;
use reqwest::{header, Client};
use serde::Deserialize;
use time::OffsetDateTime;

use super::community_id::CommunityId;
use super::member::Member;
use super::{APP_ID, REFERER};
use crate::auth::{compute_url, get_secret};
use crate::utils::{deserialize_timestamp, slug};

#[derive(Debug)]
enum PageState {
    Inital,
    Next(String),
    Done,
}

#[derive(Debug)]
pub enum Tab {
    ArtistPosts,
    Lives,
}

#[derive(Debug)]
pub struct ArtistPosts {
    all_ids: VecDeque<ArtistPostShort>,
    community_id: CommunityId,
    auth: String,
    tab: Tab,
    min_id: Option<String>,
    // For pagination
    page_state: PageState,
}

impl Tab {
    fn url(&self, community_id: &CommunityId, after: &str) -> String {
        let endpoint = match self {
            Self::ArtistPosts => "artistTabPosts",
            Self::Lives => "liveTabPosts",
        };
        let params = format!(
            "fieldSet=postsV1&\
                limit=20&\
                pagingType=CURSOR&\
                appId={}&\
                language=en&\
                platform=WEB&\
                wpf=pc",
            APP_ID
        );
        format!(
            "/post/v1.0/community-{}/{endpoint}?{after}{params}",
            community_id.id(),
        )
    }
}

impl ArtistPosts {
    pub(crate) fn init(
        community_id: CommunityId,
        tab: Tab,
        auth: String,
        min_id: Option<String>,
    ) -> Self {
        Self {
            all_ids: VecDeque::new(),
            community_id,
            tab,
            min_id,
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

        let url = compute_url(&self.tab.url(&self.community_id, &after), &secret).await?;

        let response = client
            .get(url.as_str())
            .header(header::REFERER, REFERER)
            .header(header::AUTHORIZATION, &self.auth)
            .send()
            .await?
            .error_for_status()?;
        let post_page = response.json::<ArtistPostsResponse>().await?;

        // Update page state
        self.page_state = match post_page.paging.next_params {
            Some(next_params) => PageState::Next(next_params.after),
            None => PageState::Done,
        };

        // Fill artist posts
        for p in post_page.data {
            if self
                .min_id
                .as_ref()
                .map(|id| id == &p.post_id)
                .unwrap_or(false)
            {
                self.page_state = PageState::Done;
                return Ok(());
            }
            self.all_ids.push_back(p);
        }

        Ok(())
    }
}

#[derive(Deserialize, Debug)]
struct ArtistPostsResponse {
    paging: Paging,
    data: Vec<ArtistPostShort>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Paging {
    next_params: Option<NextParams>,
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
    pub plain_body: String,
    pub author: Member,
    pub section_type: String,
}

impl ArtistPostShort {
    pub fn slug(&self) -> Result<String> {
        let body = match self.section_type.to_lowercase().as_ref() {
            "live" => None,
            _ => Some(self.plain_body.as_str()),
        };
        slug(&self.time, &self.post_id, &self.author, body)
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;

    use futures::stream::StreamExt;

    use super::*;
    use crate::utils::{setup, LOGIN_INFO};

    #[tokio::test]
    async fn artist_posts_paging() {
        let client = Client::new();
        let auth = LOGIN_INFO.get_or_init(setup()).await;
        let mut artist_posts =
            ArtistPosts::init(CommunityId::new(14), Tab::ArtistPosts, auth.clone(), None);
        let posts_stream = artist_posts.as_stream(&client).await;
        futures::pin_mut!(posts_stream);
        let mut ids = HashSet::new();
        for _ in 0..30 {
            let post = posts_stream.next().await.unwrap().unwrap();
            dbg!(&post.post_id);
            assert!(!ids.contains(&post.post_id));
            ids.insert(post.post_id);
        }
    }

    #[tokio::test]
    async fn lives_paging() {
        let client = Client::new();
        let auth = LOGIN_INFO.get_or_init(setup()).await;
        let mut artist_posts =
            ArtistPosts::init(CommunityId::new(14), Tab::Lives, auth.clone(), None);
        let posts_stream = artist_posts.as_stream(&client).await;
        futures::pin_mut!(posts_stream);
        let mut ids = HashSet::new();
        for _ in 0..30 {
            let post = posts_stream.next().await.unwrap().unwrap();
            dbg!(&post.post_id);
            assert!(!ids.contains(&post.post_id));
            ids.insert(post.post_id);
        }
    }
}
