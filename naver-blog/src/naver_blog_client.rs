use std::path::Path;

use futures::TryStreamExt;
use page_turner::{PageTurner, PagesStream};
use regex::Regex;
use reqwest::{Client, Url};

use crate::download_post::NaverBlogDownloadStatus;
use crate::member_posts::GetPostsRequest;
use crate::progress_bar::ProgressBar;
use crate::NaverBlogError;

pub struct NaverBlogClient<'client> {
    pub(crate) client: &'client Client,
}

impl<'client> NaverBlogClient<'client> {
    pub fn new(client: &'client Client) -> Self {
        Self { client }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn download_member<PB: ProgressBar>(
        &self,
        member: &str,
        download_path: impl AsRef<Path>,
        filter: Option<&Regex>,
        limit: Option<usize>,
        image_type: ImageType,
        until_post: Option<u64>,
        since_post: Option<u64>,
    ) -> Result<(), NaverBlogError> {
        let stream = self.pages(GetPostsRequest::new(member.to_owned())).items();
        futures::pin_mut!(stream);
        let mut idx = 0;
        while let Some(stub) = stream.try_next().await? {
            // Only download posts in specified range
            if until_post.map(|id| stub.post_id > id).unwrap_or(false)
                || since_post.map(|id| stub.post_id < id).unwrap_or(false)
            {
                continue;
            }

            // Break if limit is enabled and reached
            if let Some(limit) = limit {
                if idx >= limit {
                    break;
                }
            }
            idx += 1;

            // Check title filter
            if let Some(re) = filter {
                if !re.is_match(&stub.title) {
                    continue;
                }
            }

            // Download the post
            let download_result = self
                .download_post::<PB>(download_path.as_ref(), member, stub.post_id, image_type)
                .await?;
            match download_result {
                NaverBlogDownloadStatus::Downloaded => {}
                NaverBlogDownloadStatus::Exists => break,
            }
        }
        Ok(())
    }

    pub async fn download_url<PB: ProgressBar>(
        &self,
        url: &str,
        download_path: impl AsRef<Path>,
        image_type: ImageType,
    ) -> Result<(), NaverBlogError> {
        let blog_id = parse_url(url).ok_or(NaverBlogError::InvalidUrl {
            url: url.to_owned(),
        })?;
        self.download_post::<PB>(download_path, &blog_id.member, blog_id.id, image_type)
            .await?;
        Ok(())
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub enum ImageType {
    #[default]
    JpegOriginal,
    WebpOriginal,
    JpegW3840,
    JpegW966,
    JpegW800,
}

static DOMAIN_BLOGFILES_NAVER_NET: &str = "blogfiles.naver.net";
static DOMAIN_POSTFILES_PSTATIC_NET: &str = "postfiles.pstatic.net";
static DOMAIN_MBLOGTHUMBPHINF_PSTATIC_NET: &str = "mblogthumb-phinf.pstatic.net";

impl ImageType {
    pub(crate) fn protocol(&self) -> &str {
        match self {
            ImageType::JpegOriginal => "http",
            _ => "https",
        }
    }

    pub(crate) fn domain(&self) -> &str {
        match self {
            ImageType::JpegOriginal => DOMAIN_BLOGFILES_NAVER_NET,
            ImageType::JpegW3840 | ImageType::JpegW966 => DOMAIN_POSTFILES_PSTATIC_NET,
            ImageType::WebpOriginal | ImageType::JpegW800 => DOMAIN_MBLOGTHUMBPHINF_PSTATIC_NET,
        }
    }

    pub(crate) fn is_handled(url: &Url) -> bool {
        match url.domain() {
            None => false,
            Some(domain) => match domain {
                s if s == DOMAIN_BLOGFILES_NAVER_NET => true,
                s if s == DOMAIN_POSTFILES_PSTATIC_NET => true,
                s if s == DOMAIN_MBLOGTHUMBPHINF_PSTATIC_NET => true,
                _ => false,
            },
        }
    }

    pub(crate) fn query(&self) -> Option<&str> {
        match self {
            ImageType::JpegOriginal => None,
            ImageType::WebpOriginal => Some("type=o_webp"),
            ImageType::JpegW3840 => Some("type=w3840"),
            ImageType::JpegW966 => Some("type=w966"),
            ImageType::JpegW800 => Some("type=w800"),
        }
    }
}

struct NaverBlogPostId {
    member: String,
    id: u64,
}

fn parse_url(url: &str) -> Option<NaverBlogPostId> {
    let url = Url::parse(url).ok()?;
    let mut path_segments = url.path_segments()?;
    let seg0 = path_segments.next()?;

    if seg0 == "PostView.naver" {
        // Query param form
        let query_pairs = url.query_pairs();
        let query_pairs: Vec<_> = query_pairs.take(2).collect();
        let member = query_pairs
            .iter()
            .find(|(k, _)| k == "blogId")?
            .1
            .to_string();
        let id = query_pairs
            .iter()
            .find(|(k, _)| k == "logNo")?
            .1
            .parse::<u64>()
            .ok()?;

        return Some(NaverBlogPostId { member, id });
    }

    let seg1 = path_segments.next()?;

    if path_segments.next().is_some() {
        return None;
    }

    // Path segment form
    let member = seg0.to_owned();
    let id = seg1.parse::<u64>().ok()?;
    Some(NaverBlogPostId { member, id })
}
