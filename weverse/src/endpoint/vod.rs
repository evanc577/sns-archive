use anyhow::Result;
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};

use super::REFERER;
use crate::auth::compute_url;
use crate::endpoint::APP_ID;

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

pub(crate) enum VideoType {
    Extension(LiveVideo),
    NoExtension(CVideo),
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiveVideo {
    #[serde(rename = "infraVideoId")]
    infra_id: String,
    #[serde(rename = "videoId")]
    video_id: u64,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MediaInfo {
    pub title: String,
}

impl VideoType {
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
    video_ids: &VideoType,
    secret: &[u8],
) -> Result<Vec<Video>> {
    // Acquire inKey
    let base_url = match video_ids {
        VideoType::Extension(e) => format!(
            "/video/v1.0/vod/{}/inKey?preview=false&appId={}&wpf=pc",
            e.video_id, APP_ID
        ),
        VideoType::NoExtension(id) => format!(
            "/cvideo/v1.0/cvideo-{}/inKey/?videoId={}&appId={}&language=en&platform=WEB&wpf=pc",
            id.post_id, id.post_id, APP_ID
        ),
    };
    let inkey_url = compute_url(&base_url, secret).await?;

    let req = match video_ids {
        VideoType::Extension(_) => client.post(inkey_url.as_str()),
        VideoType::NoExtension(_) => client.get(inkey_url.as_str()),
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
