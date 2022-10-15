use anyhow::Result;
use reqwest::{header, Client};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize};

use super::{APP_ID, REFERER};
use crate::auth::{compute_url, get_secret};

#[derive(Eq, PartialEq, Hash, Clone, Copy, Debug)]
pub struct CommunityId(u64);

impl CommunityId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn id(&self) -> u64 {
        self.0
    }
}

impl Serialize for CommunityId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("CommunityId", 1)?;
        state.serialize_field("id", &self.0)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for CommunityId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let id = u64::deserialize(deserializer)?;
        Ok(CommunityId::new(id))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommunityIdResponse {
    community_id: u64,
}

pub(crate) async fn community_id(client: &Client, artist: &str, auth: &str) -> Result<CommunityId> {
    let secret = get_secret(client).await?;

    let url = compute_url(
        &format!(
            "/community/v1.0/communityIdUrlPathByUrlPathArtistCode?appId={}&keyword={}&language=en&platform=WEB&wpf=pc",
            APP_ID, artist),
        &secret)
        .await?;

    let id = client
        .get(url.as_str())
        .header(header::REFERER, REFERER)
        .header(header::AUTHORIZATION, auth)
        .send()
        .await?
        .error_for_status()?
        .json::<CommunityIdResponse>()
        .await?
        .community_id;

    Ok(CommunityId(id))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::utils::{setup, LOGIN_INFO};

    #[tokio::test]
    async fn valid() {
        let client = Client::new();
        let auth = LOGIN_INFO.get_or_init(setup()).await;
        let id = community_id(&client, "dreamcatcher", auth).await.unwrap();
        assert_eq!(id, CommunityId(14));
    }

    #[tokio::test]
    async fn uppercase() {
        let client = Client::new();
        let auth = LOGIN_INFO.get_or_init(setup()).await;
        let id = community_id(&client, "Dreamcatcher", auth).await.unwrap();
        assert_eq!(id, CommunityId(14));
    }

    #[tokio::test]
    async fn invalid() {
        let client = Client::new();
        let auth = LOGIN_INFO.get_or_init(setup()).await;
        let res = community_id(&client, "invalidcommunity", auth).await;
        assert!(res.is_err());
    }
}
