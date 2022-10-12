use anyhow::Result;
use reqwest::Client;

use crate::auth::{login, LoginInfo};
use crate::endpoint::community_id::CommunityId;
use crate::endpoint::vod::{vod_info, VodInfo};

#[derive(Clone, Debug)]
pub struct WeverseClient<'a> {
    reqwest_client: &'a Client,
}

impl<'a> WeverseClient<'a> {
    /// Create a new WeverseClient
    pub fn new(reqwest_client: &'a Client) -> Self {
        Self { reqwest_client }
    }

    /// Fetch information about a Weverse VOD, including metadata and videos
    pub async fn vod_info(&self, vod_id: &str) -> Result<VodInfo> {
        vod_info(self.reqwest_client, vod_id).await
    }
}

#[derive(Clone, Debug)]
pub struct AuthenticatedWeverseClient<'a> {
    reqwest_client: &'a Client,
    auth: String,
}

impl<'a> AuthenticatedWeverseClient<'a> {
    /// Create a new WeverseClient
    pub async fn login(
        reqwest_client: &'a Client,
        login_info: &LoginInfo,
    ) -> Result<AuthenticatedWeverseClient<'a>> {
        let auth = login(reqwest_client, login_info).await?;
        Ok(Self {
            reqwest_client,
            auth: format!("Bearer {}", auth),
        })
    }

    /// Fetch information about a Weverse VOD, including metadata and videos
    pub async fn vod_info(&self, vod_id: &str) -> Result<VodInfo> {
        vod_info(self.reqwest_client, vod_id).await
    }

    pub async fn artist_posts(&self, community: CommunityId) -> Result<()> {
        todo!()
    }
}
