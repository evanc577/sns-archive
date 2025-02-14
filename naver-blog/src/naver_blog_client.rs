use std::path::Path;

use regex::Regex;
use reqwest::{Client, Url};

use crate::NaverBlogError;

pub struct NaverBlogClient<'client> {
    pub(crate) client: &'client Client,
}

impl<'client> NaverBlogClient<'client> {
    pub fn new(client: &'client Client) -> Self {
        Self { client }
    }

    pub async fn download_member(
        &self,
        member: &str,
        download_path: impl AsRef<Path>,
        filter: Option<&Regex>,
        limit: Option<usize>,
    ) -> Result<(), NaverBlogError> {
        todo!()
    }

    pub async fn download_url(
        &self,
        url: &str,
        download_path: impl AsRef<Path>,
    ) -> Result<(), NaverBlogError> {
        let blog_id = parse_url(url).ok_or(NaverBlogError::InvalidUrl {
            url: url.to_owned(),
        })?;
        self.download_post(download_path, &blog_id.member, blog_id.id).await?;
        Ok(())
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
