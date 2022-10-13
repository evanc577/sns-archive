use std::collections::HashMap;

use anyhow::Result;
use reqwest::{header, Client};
use serde::Deserialize;
use time::OffsetDateTime;

use super::community_id::CommunityId;
use super::{APP_ID, REFERER};
use crate::auth::{compute_url, get_secret};
use crate::utils::{deserialize_community_id, deserialize_timestamp};

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ArtistPost {
    pub attachment: PostAttachment,
    #[serde(rename = "publishedAt")]
    #[serde(deserialize_with = "deserialize_timestamp")]
    pub time: OffsetDateTime,
    pub post_type: String,
    #[serde(rename = "postId")]
    pub id: String,
    pub body: String,
    pub plain_body: String,
    pub author: Member,
    pub community: Community,
}

#[derive(Deserialize, Clone, Debug)]
pub struct PostAttachment {
    pub photo: Option<HashMap<String, Photo>>,
    pub video: Option<HashMap<String, Video>>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Photo {
    pub url: String,
    pub width: u64,
    pub height: u64,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Video {
    pub upload_info: VideoUploadInfo,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VideoUploadInfo {
    pub width: u64,
    pub height: u64,
    #[serde(rename = "videoId")]
    pub id: String,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    #[serde(rename = "memberId")]
    pub id: String,
    #[serde(deserialize_with = "deserialize_community_id")]
    pub community_id: CommunityId,
    pub profile_name: String,
    #[serde(rename = "artistOfficialProfile")]
    pub official_profile: OfficialProfile,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OfficialProfile {
    pub official_name: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Community {
    #[serde(rename = "communityId")]
    #[serde(deserialize_with = "deserialize_community_id")]
    pub id: CommunityId,
    #[serde(rename = "communityName")]
    pub name: String,
}

pub(crate) async fn post(client: &Client, auth: &str, post_id: &str) -> Result<ArtistPost> {
    let secret = get_secret(client).await?;

    let url = compute_url(
        &format!(
            "/post/v1.0/post-{}?fieldSet=postV1&appId={}&language=en&platform=WEB&wpf=pc",
            post_id, APP_ID
        ),
        &secret,
    )
    .await?;

    let post = client
        .get(url.as_str())
        .header(header::REFERER, REFERER)
        .header(header::AUTHORIZATION, auth)
        .send()
        .await?
        .error_for_status()?
        .json::<ArtistPost>()
        .await?;

    Ok(post)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::utils::{setup, LOGIN_INFO};

    #[tokio::test]
    async fn post_with_video() {
        let client = Client::new();
        let auth = LOGIN_INFO.get_or_init(setup()).await;
        let post = post(&client, auth, "1-106028137").await.unwrap();
        assert_eq!(1, post.attachment.video.unwrap().len());
    }

    #[tokio::test]
    async fn post_with_image() {
        let client = Client::new();
        let auth = LOGIN_INFO.get_or_init(setup()).await;
        let post = post(&client, auth, "1-106622307").await.unwrap();
        assert_eq!(2, post.attachment.photo.unwrap().len());
    }

    #[tokio::test]
    async fn invalid_post() {
        let client = Client::new();
        let auth = LOGIN_INFO.get_or_init(setup()).await;
        let post = post(&client, auth, "5-2849541rq3").await;
        assert!(post.is_err());
    }
}
