use std::collections::VecDeque;

use anyhow::Result;
use futures::Stream;
use reqwest::{header::COOKIE, Client};
use serde::Deserialize;

use crate::weibo_post::WeiboPost;

pub struct WeiboPosts {
    user: u64,
    cookie: String,
    fetch_state: FetchState,
}

struct FetchState {
    errored: bool,
    page: u64,
    posts: VecDeque<WeiboPost>,
}

impl Default for FetchState {
    fn default() -> Self {
        Self {
            errored: false,
            page: 1,
            posts: Default::default(),
        }
    }
}

impl WeiboPosts {
    pub(crate) async fn auth(user: u64, cookie: String) -> Self {
        Self {
            user,
            cookie,
            fetch_state: Default::default(),
        }
    }

    pub async fn as_stream<'a>(
        &'a mut self,
        client: &'a Client,
    ) -> impl Stream<Item = Result<WeiboPost>> + 'a {
        self.fetch_state = Default::default();
        futures::stream::unfold(self, |state| async {
            // Stop if previously errored
            if state.fetch_state.errored {
                return None;
            }

            // Return next post if it exists
            if let Some(post) = state.fetch_state.posts.pop_front() {
                return Some((Ok(post), state));
            }

            match get_page(client, &state.cookie, state.user, state.fetch_state.page).await {
                Ok(p) => {
                    state.fetch_state.posts.extend(p.into_iter());
                    state.fetch_state.page += 1;
                }
                Err(e) => {
                    state.fetch_state.errored = true;
                    return Some((Err(e), state));
                }
            }

            // Return next post if it exists
            if let Some(post) = state.fetch_state.posts.pop_front() {
                return Some((Ok(post), state));
            }

            None
        })
    }
}

pub async fn get_page(
    client: &Client,
    cookie: &str,
    uid: u64,
    page: u64,
) -> Result<Vec<WeiboPost>> {
    static URL: &str = "https://weibo.com/ajax/statuses/mymblog";

    #[derive(Deserialize, Debug)]
    struct Mymblog {
        data: WeiboData,
    }

    #[derive(Deserialize, Debug)]
    struct WeiboData {
        list: Vec<WeiboPost>,
    }

    let posts = client
        .get(URL)
        .query(&[("uid", &uid.to_string()), ("page", &page.to_string())])
        .header(COOKIE, format!("SUB={}", cookie))
        .send()
        .await?
        .json::<Mymblog>()
        .await?
        .data
        .list;

    Ok(posts)
}
