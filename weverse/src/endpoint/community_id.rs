use anyhow::Result;
use reqwest::{header, Client};
use serde::Deserialize;

use crate::auth::{compute_url, get_secret};

use super::{APP_ID, REFERER};

#[derive(Eq, PartialEq, Hash, Clone, Copy, Debug)]
pub struct CommunityId(u64);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommunityIdResponse {
    community_id: u64,
}

pub(crate) async fn community_id(client: &Client, artist: &str) -> Result<CommunityId> {
    let secret = get_secret(client).await?;

    let url = compute_url(
        &format!(
            "/community/v1.0/communityIdUrlPathByUrlPathArtistCode?appId={}&keyword={}&language=en&platform=WEB&wpf=pc",
            APP_ID, artist),
        &secret)
        .await?;
    dbg!(url.as_str());
    let id = client
        .get(url.as_str())
        .header(header::REFERER, REFERER)
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
    use crate::WeverseClient;

    use super::*;

    #[tokio::test]
    async fn valid() {
        let client = Client::new();
        let id = community_id(&client, "dreamcatcher").await.unwrap();
        assert_eq!(id, CommunityId(14));
    }

    #[tokio::test]
    async fn uppercase() {
        let client = Client::new();
        let id = community_id(&client, "Dreamcatcher").await.unwrap();
        assert_eq!(id, CommunityId(14));
    }

    #[tokio::test]
    async fn invalid() {
        let client = Client::new();
        let res = community_id(&client, "invalidcommunity").await;
        assert!(res.is_err());
    }
}
