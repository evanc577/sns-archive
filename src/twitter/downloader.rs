use crate::config::twitter::TwitterConfig;
use super::response_helpers;
use super::tweet::Tweet;

use anyhow::{Context, Result};
use chrono::{offset, DateTime, FixedOffset};
use futures::stream::StreamExt;
use itertools::Itertools;
use reqwest::{Client, ClientBuilder};
use std::ffi::OsStr;
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::time::{sleep, Duration};
use url::Url;

pub struct DownloadClient<'a> {
    client: Client,
    config: &'a TwitterConfig,
}

#[derive(Debug)]
struct DownloadError {
    text: String,
}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.text)
    }
}

impl std::error::Error for DownloadError {}

impl DownloadError {
    fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
        }
    }
}

impl<'a> DownloadClient<'a> {
    pub fn new(config: &'a TwitterConfig) -> DownloadClient {
        DownloadClient {
            client: ClientBuilder::new()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap(),
            config,
        }
    }

    pub async fn download_tweets(
        &self,
        tweets: impl Iterator<Item = &Tweet>,
        user_filter: Option<&str>,
    ) {
        let mut errors =
            futures::stream::iter(tweets.into_iter().map(|t| self.save_tweet(t, user_filter)))
                .buffer_unordered(20)
                .collect::<Vec<_>>()
                .await
                .into_iter()
                .filter_map(|r| r.err())
                .peekable();

        if errors.peek().is_some() {
            eprintln!("\nErrors:");
            for error in errors {
                eprintln!();
                for cause in error.chain() {
                    eprintln!("{}", cause);
                }
            }
        }

        println!("\nDownload complete");
    }

    async fn save_tweet(&self, tweet: &Tweet, user_filter: Option<&str>) -> Result<()> {
        if let Some(filter) = user_filter {
            if filter.to_lowercase() != tweet.user.screen_name.to_lowercase() {
                return Ok(());
            }
        }

        // Parse tweet timestamp
        let offset = offset::FixedOffset::east(self.config.timezone_offset);
        let datetime = DateTime::parse_from_str(tweet.created_at.as_str(), "%a %b %d %T %z %Y")
            .context(format!(
                "{}: Failed to parse date: {}",
                &tweet.id, &tweet.created_at
            ))?
            .with_timezone(&offset);
        let date = datetime.format("%Y%m%d");

        // Generate base name
        let base_name = format!("{}_{}_{}", date, &tweet.id, &tweet.user.screen_name);
        let target_dir = Path::new(&self.config.download_path)
            .join(&tweet.user.screen_name)
            .join(&base_name);

        // Skip download if target directory already exists
        if target_dir.exists() {
            return Ok(());
        }

        // Create temporary directory
        let temp_dir = tempfile::Builder::new()
            .prefix(&format!("tweet_{}", &tweet.id))
            .tempdir()
            .context(format!(
                "{}: Failed to create temporary directory",
                &tweet.id
            ))?;

        // Prepare to download and write content
        println!("Downloading {}", base_name);
        self.write_tweet_text(tweet, &temp_dir.path(), &base_name, &datetime)
            .await?;
        let download_result = self
            .download_media(tweet, &temp_dir.path(), &base_name)
            .await;

        // move temp_dir to target_dir
        let options = fs_extra::dir::CopyOptions {
            copy_inside: true,
            ..fs_extra::dir::CopyOptions::new()
        };
        fs_extra::move_items(&[&temp_dir.path()], &target_dir, &options).context(format!(
            "{}: Failed to move {:?} to {:?}",
            &tweet.id,
            &temp_dir.path(),
            &target_dir
        ))?;

        download_result
    }

    async fn write_tweet_text(
        &self,
        tweet: &Tweet,
        dir_path: impl AsRef<OsStr>,
        base_name: &str,
        datetime: &DateTime<FixedOffset>,
    ) -> Result<()> {
        // Truncate tweet text to display range, expand urls
        let mut body = self.unicode_slice(
            &tweet.full_text,
            [
                tweet.display_text_range[0] as usize,
                tweet.display_text_range[1] as usize,
            ],
        );
        for url_entity in &tweet.entities.urls {
            body = body.replace(&url_entity.url, &url_entity.expanded_url);
        }

        // Generate text to write to file
        let mut data = "".to_string();
        data.push_str(&format!(
            "url: https://twitter.com/i/status/{}\n",
            &tweet.id
        ));
        data.push_str(&format!("name: {}\n", &tweet.user.name));
        data.push_str(&format!("screen_name: {}\n", &tweet.user.screen_name));
        data.push_str(&format!("created_at: {}\n", &datetime.format("%+")));
        data.push('\n');
        data.push_str(&format!("{}\n", body));

        // Write text to file
        let file_name = Path::new(&dir_path).join(format!("{}.txt", &base_name));
        let mut f = File::create(&file_name).await.context(format!(
            "{}: Failed to create file {:?}",
            &tweet.id, &file_name
        ))?;
        f.write_all(data.as_bytes()).await.context(format!(
            "{}: Failed to write to file {:?}",
            &tweet.id, &file_name
        ))?;

        Ok(())
    }

    fn unicode_slice(&self, input: &str, bounds: [usize; 2]) -> String {
        input
            .chars()
            .skip(bounds[0])
            .take(bounds[1].saturating_sub(bounds[0]))
            .collect()
    }

    async fn download_media(
        &self,
        tweet: &Tweet,
        dir_path: impl AsRef<OsStr>,
        base_name: &str,
    ) -> Result<()> {
        let mut error_urls = vec![];

        if let Some(e) = &tweet.extended_entities {
            let photos = e
                .media
                .iter()
                .filter(|m| m.media_type == "photo")
                .collect::<Vec<_>>();
            let videos = e
                .media
                .iter()
                .filter_map(|m| m.video_info.as_ref())
                .collect::<Vec<_>>();

            // Download photos
            for (photo, i) in photos.iter().zip(1_usize..) {
                let file_name = Path::new(&dir_path).join(format!(
                    "{}_img{}.{}",
                    &base_name,
                    i,
                    self.url_file_ext(&photo.media_url_https)?
                ));
                let res = self
                    .download_file(&photo.media_url_https, &[("name", "orig")], &file_name)
                    .await
                    .context(format!(
                        "{}: Failed to download file {}",
                        &tweet.id, &photo.media_url_https
                    ));
                if let Err(err) = res {
                    for e in err.chain() {
                        error_urls.push(format!("{}", e));
                    }
                }
            }

            // Download videos
            for (video, i) in videos.iter().zip(1_usize..) {
                // Find highest bitrate video
                let max_video = match video
                    .variants
                    .iter()
                    .max_by(|a, b| a.bitrate.cmp(&b.bitrate))
                {
                    Some(v) => v,
                    None => continue,
                };
                let file_name = Path::new(&dir_path).join(format!(
                    "{}_vid{}.{}",
                    &base_name,
                    i,
                    self.url_file_ext(&max_video.url)?
                ));
                let res = self
                    .download_file(&max_video.url, &[], &file_name)
                    .await
                    .context(format!(
                        "{}: Failed to download file {}",
                        &tweet.id, &max_video.url
                    ));
                if let Err(err) = res {
                    for e in err.chain() {
                        error_urls.push(format!("{}", e));
                    }
                }
            }
        }

        if !error_urls.is_empty() {
            let err: String = error_urls
                .into_iter()
                .intersperse("\n".to_string())
                .collect();
            return Err(DownloadError::new(&err).into())
        }

        Ok(())
    }

    async fn download_file(
        &self,
        url: &str,
        query: &[(&str, &str)],
        path: impl AsRef<Path> + AsRef<OsStr>,
    ) -> Result<()> {
        loop {
            let resp = self.client.get(url).query(&query).send().await?;

            if resp.status().is_success() {
                let mut file = File::create(&path).await?;
                let mut stream = resp.bytes_stream();
                while let Some(b) = stream.next().await {
                    let chunk = b?;
                    file.write_all(&chunk).await?;
                }
                break;
            }

            if let Some(duration) = response_helpers::check_rate_limit(&resp) {
                eprintln!("Rate limit hit, sleeping for {:?}", duration);
                sleep(duration).await;
                continue;
            }

            if resp.status() == reqwest::StatusCode::FORBIDDEN {
                return Err(DownloadError::new("403 forbidden").into());
            }

            return Err(DownloadError::new(format!("{:?}", &resp).as_str()).into());
        }

        Ok(())
    }

    fn url_file_ext(&self, url: &str) -> Result<String> {
        let parsed = Url::parse(url).unwrap();
        let path = parsed.path();
        let error_text = format!("Failed to parse extension for {}", url);
        Ok(Path::new(&path)
            .extension()
            .ok_or_else(|| DownloadError::new(&error_text))?
            .to_str()
            .ok_or_else(|| DownloadError::new(&error_text))?
            .to_string())
    }
}
