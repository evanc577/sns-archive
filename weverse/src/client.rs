use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use reqwest::Client;

use crate::auth::{login, LoginInfo};
use crate::endpoint::artist_tab_posts::{ArtistPosts, Tab};
use crate::endpoint::community_id::{community_id, CommunityId};
use crate::endpoint::moments::Moments;
use crate::endpoint::post::{post, ArtistPost};
use crate::endpoint::vod::{vod_info, VodInfo};

#[derive(Clone, Debug)]
pub struct AuthenticatedWeverseClient<'a> {
    reqwest_client: &'a Client,
    auth: String,
    community_id_map: Arc<Mutex<HashMap<String, CommunityId>>>,
}

impl<'a> AuthenticatedWeverseClient<'a> {
    /// Create a new AuthenticatedWeverseClient
    pub async fn login(
        reqwest_client: &'a Client,
        login_info: &LoginInfo,
    ) -> Result<AuthenticatedWeverseClient<'a>> {
        let auth = login(reqwest_client, login_info).await?;
        Ok(Self {
            reqwest_client,
            auth,
            community_id_map: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Fetch information about a Weverse VOD, including metadata and videos
    pub async fn vod_info(&self, vod_id: &str) -> Result<VodInfo> {
        vod_info(self.reqwest_client, &self.auth, vod_id).await
    }

    pub async fn artist_posts(
        &self,
        artist: &str,
        min_id: Option<String>,
        limit: Option<usize>,
    ) -> Result<ArtistPosts> {
        let community_id = self.get_community_id(artist).await?;
        Ok(ArtistPosts::init(
            community_id,
            Tab::ArtistPosts,
            self.auth.clone(),
            min_id,
            limit,
        ))
    }

    pub async fn artist_moments(&self, artist: &str) -> Result<Vec<ArtistPost>> {
        let community_id = self.get_community_id(artist).await?;
        Moments::get_latest_moments(self.reqwest_client, &self.auth, community_id).await
    }

    pub async fn lives(
        &self,
        artist: &str,
        min_id: Option<String>,
        limit: Option<usize>,
    ) -> Result<ArtistPosts> {
        let community_id = self.get_community_id(artist).await?;
        Ok(ArtistPosts::init(
            community_id,
            Tab::Lives,
            self.auth.clone(),
            min_id,
            limit,
        ))
    }

    pub async fn post(&self, post_id: &str) -> Result<ArtistPost> {
        post(self.reqwest_client, &self.auth, post_id).await
    }

    async fn get_community_id(&self, artist: &str) -> Result<CommunityId> {
        if let Some(id) = self.community_id_map.lock().unwrap().get(artist) {
            return Ok(*id);
        }

        let id = community_id(self.reqwest_client, artist, &self.auth).await?;
        self.community_id_map
            .lock()
            .unwrap()
            .insert(artist.to_string(), id);
        Ok(id)
    }
}
