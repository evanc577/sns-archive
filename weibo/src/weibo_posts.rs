use std::collections::VecDeque;
use std::time::Duration;

use anyhow::{anyhow, Result};
use futures::Stream;
use reqwest::header::COOKIE;
use reqwest::{Client, StatusCode};
use serde::Deserialize;

use crate::weibo_auth::WeiboAuth;
use crate::weibo_post::WeiboPost;

pub struct WeiboPosts {
    user: u64,
    auth: WeiboAuth,
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
    pub(crate) async fn auth(user: u64, auth: WeiboAuth) -> Self {
        Self {
            user,
            auth,
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

            match get_page(client, &state.auth, state.user, state.fetch_state.page).await {
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
    auth: &WeiboAuth,
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

    let mut retry_414 = 0;
    let mut posts = loop {
        let resp = client
            .get(URL)
            .query(&[("uid", &uid.to_string()), ("page", &page.to_string())])
            .header(COOKIE, format!("SUB={}", auth.cookies))
            .send()
            .await?;

        if resp.status().as_u16() == StatusCode::URI_TOO_LONG {
            if retry_414 == 0 {
                eprintln!("status code 414");
            }
            if retry_414 >= 10 {
                return Err(anyhow!("error 414"));
            }
            retry_414 += 1;
            tokio::time::sleep(Duration::from_secs(5)).await;
            continue;
        }

        break resp.json::<Mymblog>().await?.data.list;
    };

    for p in posts.iter_mut() {
        p.set_tid(auth.tid.clone());
    }

    Ok(posts)
}
