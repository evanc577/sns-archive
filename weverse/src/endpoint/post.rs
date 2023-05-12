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
use time::OffsetDateTime;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use super::community_id::CommunityId;
use super::member::Member;
use super::vod::{vod_videos, CVideo, VideoIds};
use super::{APP_ID, REFERER};
use crate::auth::{compute_url, get_secret};
use crate::error::WeverseError;
use crate::utils::{deserialize_timestamp, slug};

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ArtistPost {
    attachment: PostAttachment,
    #[serde(flatten)]
    extension: Extension,
    #[serde(rename = "publishedAt")]
    #[serde(deserialize_with = "deserialize_timestamp")]
    #[serde(serialize_with = "rfc3339::serialize")]
    time: OffsetDateTime,
    pub section_type: String,
    #[serde(rename = "postId")]
    id: String,
    body: String,
    plain_body: String,
    author: Member,
    community: Community,
    author_moment_posts: Option<AuthorMomentPosts>,
    #[serde(skip)]
    auth: String,
}

/// Maps id to photo/video
#[derive(Deserialize, Serialize, Clone, Debug)]
struct PostAttachment {
    photo: Option<HashMap<String, Photo>>,
    video: Option<HashMap<String, Video>>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(tag = "postType", content = "extension")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum Extension {
    Normal(ExtensionNormal),
    Moment(ExtensionMoment),
    MomentW1(ExtensionMomentW1),
    Video(ExtensionVideo),
}

impl Extension {
    fn moment(&self) -> Option<&Moment> {
        match self {
            Self::Moment(m) => Some(&m.moment),
            Self::MomentW1(m) => Some(&m.moment_w1),
            _ => None,
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct ExtensionNormal {}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct ExtensionMoment {
    moment: Moment,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct ExtensionMomentW1 {
    moment_w1: Moment,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
enum Moment {
    BgImage {
        #[serde(rename = "backgroundImageUrl")]
        url: String,
    },
    Photo {
        photo: Photo,
    },
    Video {
        video: Video,
    },
}

impl Moment {
    fn photo_url(&self) -> Option<&str> {
        match self {
            Self::Photo { photo } => Some(&photo.url),
            Self::BgImage { url } => Some(&url),
            _ => None,
        }
    }

    fn video(&self) -> Option<&Video> {
        match self {
            Self::Video { video } => Some(&video),
            _ => None,
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct ExtensionVideo {
    video: LiveVideo,
    media_info: MediaInfo,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct LiveVideo {
    #[serde(rename = "infraVideoId")]
    infra_id: String,
    #[serde(rename = "videoId")]
    video_id: u64,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct MediaInfo {
    title: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
struct Photo {
    #[serde(alias = "backgroundImageUrl")]
    url: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct Video {
    upload_info: VideoUploadInfo,
    #[serde(rename = "videoId")]
    id: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct VideoUploadInfo {
    width: u64,
    height: u64,
    #[serde(rename = "videoId")]
    id: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum PostType {
    Normal,
    Moment,
    /// Legacy moment
    MomentW1,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
struct Community {
    #[serde(rename = "communityId")]
    id: CommunityId,
    #[serde(rename = "communityName")]
    name: String,
}

/// Contains some of the artists moments before and after the current one
#[derive(Deserialize, Serialize, Clone, Debug)]
struct AuthorMomentPosts {
    data: Vec<MomentData>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct MomentData {
    #[serde(rename = "postId")]
    id: String,
    #[serde(rename = "publishedAt")]
    #[serde(deserialize_with = "deserialize_timestamp")]
    #[serde(serialize_with = "rfc3339::serialize")]
    time: OffsetDateTime,
    author: Member,
    plain_body: String,
}

/// Fetch a post given with a post ID
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

    let mut post = client
        .get(url.as_str())
        .header(header::REFERER, REFERER)
        .header(header::AUTHORIZATION, auth)
        .send()
        .await?
        .error_for_status()?
        .json::<ArtistPost>()
        .await?;
    post.auth = auth.to_owned();

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
        match self.section_type.to_lowercase().as_str() {
            "live" => {
                let dir = directory.as_ref();
                let res = self
                    .download_all_videos(client, &self.auth, dir, true)
                    .await;
                if res.iter().any(|r| r.is_err()) {
                    return Err(WeverseError::Download(self.id.clone()).into());
                }
            }
            _ => {
                let (info_res, photos_res, videos_res) = futures::join!(
                    self.write_info(directory.as_ref()),
                    self.download_all_photos(client, directory.as_ref()),
                    self.download_all_videos(client, &self.auth, directory.as_ref(), false),
                );

                if info_res.is_err()
                    || photos_res.iter().any(|r| r.is_err())
                    || videos_res.iter().any(|r| r.is_err())
                {
                    return Err(WeverseError::Download(self.id.clone()).into());
                }
            }
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
        let text = if let Extension::Video(ExtensionVideo { media_info, .. }) = &self.extension {
            &media_info.title
        } else {
            &self.plain_body
        };
        slug(&self.time, &self.id, &self.author, Some(text))
    }
}

impl ArtistPost {
    /// Returns the next newest moment after the current one
    pub fn next_moment_id(&self) -> Option<String> {
        self.author_moment_posts.as_ref().and_then(|mps| {
            mps.data
                .iter()
                .skip_while(|m| m.id != self.id)
                .nth(1)
                .map(|m| m.id.clone())
        })
    }

    /// Write all data as a json file
    async fn write_info(&self, directory: impl AsRef<Path>) -> Result<()> {
        let info = serde_json::to_vec_pretty(self)?;
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
        // Download both regular and moments photos
        let photos = self.photos().map(|p| p.url.clone()).chain(
            self.extension
                .moment()
                .and_then(|m| m.photo_url().map(|u| u.to_owned()))
                .into_iter(),
        );
        futures::stream::iter(photos)
            .enumerate()
            .map(|(i, p)| self.download_photo(client, p, i, directory.as_ref()))
            .buffered(usize::MAX)
            .collect()
            .await
    }

    async fn download_all_videos(
        &self,
        client: &Client,
        auth: &str,
        directory: impl AsRef<Path>,
        is_live: bool,
    ) -> Vec<Result<()>> {
        // Download both regular and moments videos
        let videos = self.videos().chain(
            self.extension
                .moment()
                .and_then(|m| m.video().map(|v| v.to_owned()))
                .into_iter(),
        );
        futures::stream::iter(videos)
            .enumerate()
            .map(|(i, v)| self.download_video(client, auth, v, i, directory.as_ref(), is_live))
            .buffered(usize::MAX)
            .collect()
            .await
    }

    async fn download_photo(
        &self,
        client: &Client,
        photo_url: impl AsRef<str>,
        idx: usize,
        directory: impl AsRef<Path>,
    ) -> Result<()> {
        let url = Url::parse(photo_url.as_ref())?;
        let ext = url
            .path()
            .rsplit_once('.')
            .map(|(_, ext)| ext)
            .unwrap_or("jpg");
        let filename = format!("{}-img{:02}.{}", self.slug()?, idx + 1, ext);
        let path = directory.as_ref().join(filename);
        streamed_download(client, photo_url.as_ref(), path).await
    }

    async fn download_video(
        &self,
        client: &Client,
        auth: &str,
        video: Video,
        idx: usize,
        directory: impl AsRef<Path>,
        is_live: bool,
    ) -> Result<()> {
        let video_ids = VideoIds::NoExtension(CVideo {
            post_id: video.id,
            infra_id: video.upload_info.id,
        });
        let secret = get_secret(client).await.unwrap();
        let vod_info = vod_videos(client, auth, &video_ids, &secret).await.unwrap();
        let video_url = &vod_info.iter().max().unwrap().source;
        let url = Url::parse(video_url)?;
        let ext = url
            .path()
            .rsplit_once('.')
            .map(|(_, ext)| ext)
            .unwrap_or("mp4");
        let filename = if is_live {
            format!("{}.{}", self.slug()?, ext)
        } else {
            format!("{}-vid{:02}.{}", self.slug()?, idx + 1, ext)
        };
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
        assert_eq!(post.slug().unwrap(), "20220927-1-106028137-DAMI-이런걸 왜 찍었던 거지🤔");
        assert_eq!(1, post.attachment.video.unwrap().len());
    }

    #[tokio::test]
    async fn post_with_image() {
        let client = Client::new();
        let auth = LOGIN_INFO.get_or_init(setup()).await;
        let post = post(&client, auth, "1-106622307").await.unwrap();
        assert_eq!(post.slug().unwrap(), "20221010-1-106622307-HANDONG-내일이다!!오또캐~~~🫶🫶🫶🫶");
        assert_eq!(2, post.attachment.photo.unwrap().len());
    }

    #[tokio::test]
    async fn invalid_post() {
        let client = Client::new();
        let auth = LOGIN_INFO.get_or_init(setup()).await;
        let post = post(&client, auth, "5-2849541rq3").await;
        assert!(post.is_err());
    }

    #[tokio::test]
    async fn moment_post_video() {
        let client = Client::new();
        let auth = LOGIN_INFO.get_or_init(setup()).await;
        let post = post(&client, auth, "4-111010672").await.unwrap();
        assert_eq!(post.slug().unwrap(), "20230111-4-111010672-GAHYEON-우리색 하늘");
        assert!(!post.author_moment_posts.unwrap().data.is_empty());
        assert!(matches!(post.extension, Extension::Moment(_)));
        assert!(post.extension.moment().unwrap().video().is_some());
    }

    #[tokio::test]
    async fn moment_post_photo() {
        let client = Client::new();
        let auth = LOGIN_INFO.get_or_init(setup()).await;
        let post = post(&client, auth, "2-247595").await.unwrap();
        assert_eq!(post.slug().unwrap(), "20220606-2-247595-YOOHYEON-여러분 비편열었다근데 일반게시물 같은 이 기분은…뭐지..왜 다들 비밀로 안써!비밀");
        assert!(!post.author_moment_posts.unwrap().data.is_empty());
        assert!(matches!(post.extension, Extension::MomentW1(_)));
        assert!(post.extension.moment().unwrap().photo_url().is_some());
    }

    #[tokio::test]
    async fn moment_post_bgimage() {
        let client = Client::new();
        let auth = LOGIN_INFO.get_or_init(setup()).await;
        let post = post(&client, auth, "1-14373893").await.unwrap();
        assert_eq!(post.slug().unwrap(), "20220512-1-14373893-JI U-내 사랑❤️오늘도 수고해또❤️잘자❤️");
        assert!(!post.author_moment_posts.unwrap().data.is_empty());
        assert!(matches!(post.extension, Extension::MomentW1(_)));
        assert!(post.extension.moment().unwrap().photo_url().is_some());
    }

    #[tokio::test]
    async fn next_moment() {
        let client = Client::new();
        let auth = LOGIN_INFO.get_or_init(setup()).await;
        let post = post(&client, auth, "2-103571496").await.unwrap();
        assert_eq!(post.next_moment_id(), Some(String::from("2-103510239")))
    }

    #[tokio::test]
    async fn live() {
        let client = Client::new();
        let auth = LOGIN_INFO.get_or_init(setup()).await;
        let post = post(&client, auth, "0-119057265").await.unwrap();
        assert_eq!(post.slug().unwrap(), "20230510-0-119057265-YOOHYEON-안농");
        assert!(matches!(post.extension, Extension::Video(_)));
    }
}
