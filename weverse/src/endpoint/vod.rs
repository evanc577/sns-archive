use anyhow::Result;
use reqwest::{header, Client, Url};
use serde::Deserialize;
use time::OffsetDateTime;

use super::REFERER;
use crate::auth::{compute_url, get_secret};
use crate::endpoint::APP_ID;
use crate::utils::deserialize_timestamp;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InKeyResponse {
    in_key: String,
}

#[derive(Deserialize, Debug)]
struct VodResponse {
    videos: Videos,
}

#[derive(Deserialize, Debug)]
struct Videos {
    list: Vec<Video>,
}

/// VOD video information
#[derive(Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct Video {
    /// Source URL
    pub source: String,
    /// File size in bytes
    pub size: u64,
    /// Video encoding
    #[serde(rename = "encodingOption")]
    pub encoding: Encoding,
}

impl Ord for Video {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.encoding.cmp(&other.encoding)
    }
}

impl PartialOrd for Video {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Video encode settings
#[derive(Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct Encoding {
    /// Video width in pixels
    pub width: u64,
    /// Video height in pixels
    pub height: u64,
}

impl PartialOrd for Encoding {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Encoding {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let size = |e: &Self| e.width * e.height;
        size(self).cmp(&size(other))
    }
}

pub enum VideoIds {
    Extension(ExtensionVideo),
    NoExtension(CVideo),
}

impl VideoIds {
    fn infra_id(&self) -> &str {
        match self {
            Self::Extension(e) => e.infra_id.as_str(),
            Self::NoExtension(c) => c.infra_id.as_str(),
        }
    }
}

pub struct CVideo {
    pub post_id: String,
    pub infra_id: String,
}

pub(crate) async fn vod_videos(
    client: &Client,
    auth: &str,
    video_ids: &VideoIds,
    secret: &[u8],
) -> Result<Vec<Video>> {
    // Acquire inKey
    let base_url = match video_ids {
        VideoIds::Extension(e) => format!(
            "/video/v1.0/vod/{}/inKey?preview=false&appId={}&wpf=pc",
            e.video_id, APP_ID
        ),
        VideoIds::NoExtension(id) => format!(
            "/cvideo/v1.0/cvideo-{}/inKey/?videoId={}&appId={}&language=en&platform=WEB&wpf=pc",
            id.post_id, id.post_id, APP_ID
        ),
    };
    let inkey_url = compute_url(&base_url, secret).await?;

    let req = match video_ids {
        VideoIds::Extension(_) => client.post(inkey_url.as_str()),
        VideoIds::NoExtension(_) => client.get(inkey_url.as_str()),
    };

    let in_key = req
        .header(header::REFERER, REFERER)
        .header(header::AUTHORIZATION, auth)
        .send()
        .await?
        .error_for_status()?
        .json::<InKeyResponse>()
        .await?
        .in_key;

    // Get vod info
    let url = format!(
        "https://global.apis.naver.com/rmcnmv/rmcnmv/vod/play/v2.0/{}",
        video_ids.infra_id()
    );
    let mut videos = client
        .get(&url)
        .query(&[("key", in_key.as_str())])
        .send()
        .await?
        .error_for_status()?
        .json::<VodResponse>()
        .await?
        .videos
        .list;
    videos.sort();

    Ok(videos)
}

/// General VOD info
#[derive(Debug, Clone)]
pub struct VodInfo {
    pub title: String,
    pub id: String,
    pub url: Url,
    pub time: OffsetDateTime,
    pub author: String,
    pub videos: Vec<Video>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct VodInfoResponse {
    title: String,
    post_id: String,
    #[serde(rename = "shareUrl")]
    url: String,
    #[serde(rename = "publishedAt")]
    #[serde(deserialize_with = "deserialize_timestamp")]
    time: OffsetDateTime,
    author: Author,
    extension: Extension,
}

#[derive(Deserialize, Debug)]
struct Author {
    #[serde(rename = "profileName")]
    name: String,
}

#[derive(Deserialize, Debug)]
struct Extension {
    video: ExtensionVideo,
}

#[derive(Deserialize, Debug)]
pub struct ExtensionVideo {
    #[serde(rename = "infraVideoId")]
    infra_id: String,
    #[serde(rename = "videoId")]
    video_id: u64,
}

pub(crate) async fn vod_info(client: &Client, auth: &str, vod_id: &str) -> Result<VodInfo> {
    let secret = get_secret(client).await?;

    // Get VOD info
    let url = compute_url(
        &format!(
            "/post/v1.0/post-{}?fieldSet=postV1&appId={}&language=en&platform=WEB&wpf=pc",
            vod_id, APP_ID
        ),
        &secret,
    )
    .await?;
    let resp = client
        .get(url.as_str())
        .header(header::REFERER, REFERER)
        .header(header::AUTHORIZATION, auth)
        .send()
        .await?
        .error_for_status()?
        .json::<VodInfoResponse>()
        .await?;

    // Get videos
    let videos = vod_videos(
        client,
        auth,
        &VideoIds::Extension(resp.extension.video),
        &secret,
    )
    .await?;

    let info = VodInfo {
        title: resp.title,
        id: resp.post_id,
        url: Url::parse(&resp.url)?,
        time: resp.time,
        author: resp.author.name,
        videos,
    };

    Ok(info)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::utils::{setup, LOGIN_INFO};

    #[tokio::test]
    async fn get_vod_info() {
        let client = Client::new();
        let auth = LOGIN_INFO.get_or_init(setup()).await;
        let vod_info = vod_info(&client, auth, "2-106178776").await.unwrap();
        dbg!(vod_info);
    }

    #[tokio::test]
    async fn get_vod_info_vlive() {
        let client = Client::new();
        let auth = LOGIN_INFO.get_or_init(setup()).await;
        let vod_info = vod_info(&client, auth, "1-105466775").await.unwrap();
        dbg!(vod_info);
    }
}
