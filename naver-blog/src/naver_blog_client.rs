use std::path::Path;

use regex::Regex;
use reqwest::Client;
use anyhow::Result;

pub struct NaverBlogClient<'client> {
    client: &'client Client,
}

impl<'client> NaverBlogClient<'client> {
    pub fn new(client: &'client Client) -> Self {
        Self { client }
    }

    pub async fn download_member(
        &self,
        member: &str,
        download_path: impl AsRef<Path>,
        filter: &Regex,
        limit: Option<usize>,
    ) -> Result<()> {
        todo!()
    }

    pub async fn download_url(
        &self,
        url: &str,
        download_path: impl AsRef<Path>,
    ) -> Result<()> {
        todo!()
    }
}
