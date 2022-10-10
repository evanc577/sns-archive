use anyhow::Result;
use reqwest::{header, Client, Url};
use serde::{Deserialize, Deserializer};
use time::OffsetDateTime;

use crate::auth::{compute_url, get_secret};

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

#[derive(Deserialize, PartialEq, Eq, Debug)]
pub struct Video {
    pub source: String,
    pub size: u64,
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

#[derive(Deserialize, PartialEq, Eq, Debug)]
pub struct Encoding {
    pub width: u64,
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

async fn vod_videos(
    client: &Client,
    extension: &ExtensionVideo,
    secret: &[u8],
) -> Result<Vec<Video>> {
    // Acquire inKey
    let inkey_url = compute_url(
        &format!(
            "/video/v1.0/vod/{}/inKey?preview=false&appId=be4d79eb8fc7bd008ee82c8ec4ff6fd4&wpf=pc",
            extension.video_id
        ),
        secret,
    )
    .await?;
    let in_key = client
        .post(inkey_url.as_str())
        .header(header::REFERER, "https://weverse.io/")
        .send()
        .await?
        .json::<InKeyResponse>()
        .await?
        .in_key;
    dbg!(&in_key);

    // Get vod info
    let url = format!(
        "https://global.apis.naver.com/rmcnmv/rmcnmv/vod/play/v2.0/{}",
        extension.infra_id
    );
    let mut videos = client
        .get(&url)
        .query(&[("key", in_key.as_str())])
        .send()
        .await?
        .json::<VodResponse>()
        .await?
        .videos
        .list;
    videos.sort();

    Ok(videos)
}

#[derive(Debug)]
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

fn deserialize_timestamp<'de, D>(deserializer: D) -> Result<OffsetDateTime, D::Error>
where
    D: Deserializer<'de>,
{
    let ts = i128::deserialize(deserializer)? * 1_000_000;
    OffsetDateTime::from_unix_timestamp_nanos(ts).map_err(serde::de::Error::custom)
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
struct ExtensionVideo {
    #[serde(rename = "infraVideoId")]
    infra_id: String,
    #[serde(rename = "videoId")]
    video_id: u64,
}

pub async fn vod_info(client: &Client, vod_id: &str) -> Result<VodInfo> {
    let secret = get_secret(client).await?;

    // Get VOD info
    let url = compute_url(&format!("/post/v1.0/post-{}?fieldSet=postV1&appId=be4d79eb8fc7bd008ee82c8ec4ff6fd4&language=en&platform=WEB&wpf=pc", vod_id), &secret).await?;
    let resp = client
        .get(url.as_str())
        .header(header::REFERER, "https://weverse.io/")
        .send()
        .await?
        .json::<VodInfoResponse>()
        .await?;

    // Get videos
    let videos = vod_videos(client, &resp.extension.video, &secret).await?;

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

    #[tokio::test]
    async fn get_vod_info() {
        let client = Client::new();
        let vod_info = vod_info(&client, "2-106178776").await.unwrap();
        dbg!(vod_info);
    }

    #[tokio::test]
    async fn get_vod_info_vlive() {
        let client = Client::new();
        let vod_info = vod_info(&client, "1-105466775").await.unwrap();
        dbg!(vod_info);
    }
}
