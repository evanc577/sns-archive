use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use futures::StreamExt;
use lazy_static::lazy_static;
use regex::Regex;
use reqwest::{header, Client, Url};
use serde::{Deserialize, Serialize};
use sns_archive_common::{set_mtime, streamed_download, SavablePost};
use time::serde::rfc3339;
use time::{format_description, OffsetDateTime};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use unicode_segmentation::UnicodeSegmentation;

use super::community_id::CommunityId;
use super::vod::{vod_videos, CVideo, VideoIds};
use super::{APP_ID, REFERER};
use crate::auth::{compute_url, get_secret};
use crate::error::WeverseError;
use crate::utils::deserialize_timestamp;

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ArtistPost {
    pub attachment: PostAttachment,
    #[serde(rename = "publishedAt")]
    #[serde(deserialize_with = "deserialize_timestamp")]
    #[serde(serialize_with = "rfc3339::serialize")]
    pub time: OffsetDateTime,
    pub post_type: String,
    #[serde(rename = "postId")]
    pub id: String,
    pub body: String,
    pub plain_body: String,
    pub author: Member,
    pub community: Community,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct PostAttachment {
    pub photo: Option<HashMap<String, Photo>>,
    pub video: Option<HashMap<String, Video>>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Photo {
    pub url: String,
    pub width: u64,
    pub height: u64,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Video {
    pub upload_info: VideoUploadInfo,
    #[serde(rename = "videoId")]
    pub id: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VideoUploadInfo {
    pub width: u64,
    pub height: u64,
    #[serde(rename = "videoId")]
    pub id: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    #[serde(rename = "memberId")]
    pub id: String,
    pub community_id: CommunityId,
    pub profile_name: String,
    #[serde(rename = "artistOfficialProfile")]
    pub official_profile: OfficialProfile,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OfficialProfile {
    pub official_name: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Community {
    #[serde(rename = "communityId")]
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

lazy_static! {
    static ref ATTACHMENT_PHOTO_RE: Regex =
        Regex::new(r#"<w:attachment.*?type="photo".*?>"#).unwrap();
    static ref ATTACHMENT_VIDEO_RE: Regex =
        Regex::new(r#"<w:attachment.*?type="video".*?>"#).unwrap();
    static ref ATTACHMENT_ID_RE: Regex = Regex::new(r#"\bid="(?P<id>[0-9\-]+)""#).unwrap();
}

#[async_trait]
impl SavablePost for ArtistPost {
    async fn download(&self, client: &Client, directory: impl AsRef<Path> + Send) -> Result<()> {
        let (info_res, photos_res, videos_res) = futures::join!(
            self.write_info(directory.as_ref()),
            self.download_all_photos(client, directory.as_ref()),
            self.download_all_videos(client, directory.as_ref()),
        );

        if info_res.is_err()
            || photos_res.iter().any(|r| r.is_err())
            || videos_res.iter().any(|r| r.is_err())
        {
            return Err(WeverseError::Download(self.id.clone()).into());
        }

        // Set mtime on directory and all files in it
        set_mtime(&directory, &self.time)?;
        let mut read_dir = fs::read_dir(directory).await?;
        while let Some(entry) = read_dir.next_entry().await? {
            set_mtime(entry.path(), &self.time)?;
        }
        Ok(())
    }

    fn slug(&self) -> Result<String> {
        let time_str = {
            let format = format_description::parse("[year][month][day]")?;
            self.time.format(&format)?
        };
        let id = &self.id;
        let username = &self.author.official_profile.official_name;
        let body: String = UnicodeSegmentation::graphemes(self.plain_body.as_str(), true)
            .take(50)
            .collect();
        let slug = format!("{}-{}-{}-{}", time_str, id, username, body);
        let sanitize_options = sanitize_filename::Options {
            windows: true,
            ..Default::default()
        };
        let sanitized_slug = sanitize_filename::sanitize_with_options(slug, sanitize_options);
        Ok(sanitized_slug)
    }
}

impl ArtistPost {
    async fn write_info(&self, directory: impl AsRef<Path>) -> Result<()> {
        let info = serde_json::to_vec(self)?;
        let filename = directory.as_ref().join(format!("{}.json", self.slug()?));
        let mut file = fs::File::create(filename).await?;
        file.write_all(info.as_slice()).await?;
        Ok(())
    }

    async fn download_all_photos(
        &self,
        client: &Client,
        directory: impl AsRef<Path>,
    ) -> Vec<Result<()>> {
        futures::stream::iter(self.photos())
            .enumerate()
            .map(|(i, p)| self.download_photo(client, p, i, directory.as_ref()))
            .buffered(usize::MAX)
            .collect()
            .await
    }

    async fn download_all_videos(
        &self,
        client: &Client,
        directory: impl AsRef<Path>,
    ) -> Vec<Result<()>> {
        futures::stream::iter(self.videos())
            .enumerate()
            .map(|(i, v)| self.download_video(client, v, i, directory.as_ref()))
            .buffered(usize::MAX)
            .collect()
            .await
    }

    async fn download_photo(
        &self,
        client: &Client,
        photo: Photo,
        idx: usize,
        directory: impl AsRef<Path>,
    ) -> Result<()> {
        let url = Url::parse(&photo.url)?;
        let ext = url
            .path()
            .rsplit_once('.')
            .map(|(_, ext)| ext)
            .unwrap_or("jpg");
        let filename = format!("{}_img{:02}.{}", self.slug()?, idx + 1, ext);
        let path = directory.as_ref().join(filename);
        streamed_download(client, photo.url, path).await
    }

    async fn download_video(
        &self,
        client: &Client,
        video: Video,
        idx: usize,
        directory: impl AsRef<Path>,
    ) -> Result<()> {
        let video_ids = VideoIds::NoExtension(CVideo {
            post_id: video.id,
            infra_id: video.upload_info.id,
        });
        let secret = get_secret(client).await.unwrap();
        let vod_info = vod_videos(client, &video_ids, &secret).await.unwrap();
        let video_url = &vod_info.iter().max().unwrap().source;
        let url = Url::parse(video_url)?;
        let ext = url
            .path()
            .rsplit_once('.')
            .map(|(_, ext)| ext)
            .unwrap_or("mp4");
        let filename = format!("{}_vid{:02}.{}", self.slug()?, idx + 1, ext);
        let path = directory.as_ref().join(filename);
        streamed_download(client, video_url, path).await
    }

    fn photos(&self) -> impl Iterator<Item = Photo> + '_ {
        self.attachments(&ATTACHMENT_PHOTO_RE)
            .filter_map(|a| Some(ATTACHMENT_ID_RE.captures(a)?.name("id")?.as_str()))
            .filter_map(|id| self.attachment.photo.as_ref()?.get(id))
            .cloned()
    }

    fn videos(&self) -> impl Iterator<Item = Video> + '_ {
        self.attachments(&ATTACHMENT_VIDEO_RE)
            .filter_map(|a| Some(ATTACHMENT_ID_RE.captures(a)?.name("id")?.as_str()))
            .filter_map(|id| self.attachment.video.as_ref()?.get(id))
            .cloned()
    }

    fn attachments<'a>(&'a self, attachment_re: &'a Regex) -> impl Iterator<Item = &'a str> {
        attachment_re
            .captures_iter(&self.body)
            .map(|c| c.get(0).unwrap().as_str())
    }
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
