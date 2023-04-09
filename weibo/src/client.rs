use anyhow::Result;
use reqwest::Client;

use crate::weibo_auth::weibo_cookie;
use crate::weibo_posts::WeiboPosts;

pub struct WeiboClient<'a> {
    reqwest_client: &'a Client,
}

impl<'a> WeiboClient<'a> {
    /// Create a new WeiboClient
    pub async fn init(reqwest_client: &'a Client) -> Result<WeiboClient<'a>> {
        Ok(Self { reqwest_client })
    }

    /// Get stream of Weibo posts
    pub async fn posts(&self, user: u64) -> Result<WeiboPosts> {
        let auth = weibo_cookie(self.reqwest_client).await?;
        let posts = WeiboPosts::auth(user, auth).await;
        Ok(posts)
    }
}
