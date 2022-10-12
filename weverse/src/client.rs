use anyhow::Result;
use reqwest::Client;

use crate::endpoint::{vod::{vod_info, VodInfo}, community_id::CommunityId};

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
    pub fn login(reqwest_client: &'a Client, password: &str) -> Self {
        todo!()
    }

    /// Fetch information about a Weverse VOD, including metadata and videos
    pub async fn vod_info(&self, vod_id: &str) -> Result<VodInfo> {
        vod_info(self.reqwest_client, vod_id).await
    }

    pub async fn artist_posts(&self, community: CommunityId) -> Result<()> {
        todo!()
    }
}
