use anyhow::Result;
use futures::stream::{self, StreamExt};
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};

use super::community_id::CommunityId;
use super::post::{post, ArtistPost};
use super::{APP_ID, REFERER};
use crate::auth::{compute_url, get_secret};

pub struct Moments {}

impl Moments {
    pub async fn get_latest_moments(
        client: &Client,
        auth: &str,
        community_id: CommunityId,
    ) -> Result<Vec<ArtistPost>> {
        let secret = get_secret(client).await?;
        let url = compute_url(
            &format!(
                "/member/v1.0/community-{}/artistMembers?appId={}&fieldSet=artistMembersV1&language=en&platform=WEB&wpf=pc",
                community_id.id(), APP_ID
                ),
                &secret,
                )
            .await?;

        let moment_ids: Vec<_> = client
            .get(url.as_str())
            .header(header::REFERER, REFERER)
            .header(header::AUTHORIZATION, auth)
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<LatestMomentWrapper>>()
            .await?
            .into_iter()
            .filter_map(|m| m.artist_latest_moment.map(|m| m.id))
            .collect();
        let posts: Vec<_> = stream::iter(moment_ids.iter())
            .map(|id| post(client, &auth, &id))
            .buffered(20)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_>>()?;
        Ok(posts)
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct LatestMomentWrapper {
    artist_latest_moment: Option<ArtistLatestMoment>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
struct ArtistLatestMoment {
    #[serde(rename = "postId")]
    id: String,
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::utils::{setup, LOGIN_INFO};

    #[tokio::test]
    async fn latest_moments() {
        let client = Client::new();
        let auth = LOGIN_INFO.get_or_init(setup()).await;
        let community_id = CommunityId::new(14);
        let latest_moments = Moments::get_latest_moments(&client, auth, community_id)
            .await
            .unwrap();
        assert_eq!(7, latest_moments.len());
    }
}
