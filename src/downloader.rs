use crate::Config;
use crate::Tweet;

use chrono::{offset, DateTime, FixedOffset};
use futures::stream::StreamExt;
use reqwest::Client;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::UNIX_EPOCH;
use tokio::time::{sleep, Duration};
use url::Url;

pub struct DownloadClient<'a> {
    client: Client,
    config: &'a Config,
}

impl<'a> DownloadClient<'a> {
    pub fn new(config: &'a Config) -> DownloadClient {
        DownloadClient {
            client: Client::new(),
            config,
        }
    }

    pub async fn download_tweets(
        &self,
        tweets: impl Iterator<Item = &Tweet>,
        user_filter: Option<&str>,
    ) {
        futures::stream::iter(
            tweets
                .into_iter()
                .map(|t| self.download_tweet(&t, user_filter)),
        )
        .buffer_unordered(20)
        .collect::<Vec<_>>()
        .await;
    }

    async fn download_tweet(&self, tweet: &Tweet, user_filter: Option<&str>) {
        if let Some(filter) = user_filter {
            if filter.to_lowercase() != tweet.user.screen_name.to_lowercase() {
                return;
            }
        }

        let offset = offset::FixedOffset::east(self.config.timezone_offset);
        let datetime = DateTime::parse_from_str(tweet.created_at.as_str(), "%a %b %d %T %z %Y")
            .unwrap()
            .with_timezone(&offset);
        let date = datetime.format("%Y%m%d");
        let base_name = format!("{}_{}_{}", date, &tweet.id, &tweet.user.screen_name);
        let temp_name = format!("{}.temp", base_name);
        let target_dir = Path::new(&self.config.directory)
            .join(&tweet.user.screen_name)
            .join(&base_name);
        let temp_dir = Path::new(&self.config.directory)
            .join(&tweet.user.screen_name)
            .join(&temp_name);

        if target_dir.exists() {
            eprintln!("Skipping {}", &base_name);
            return;
        }

        // Prepare to download and write content
        eprintln!("Downloading {}", base_name);
        fs::create_dir_all(&temp_dir).unwrap();
        self.write_tweet_text(&tweet, &temp_dir, &base_name, &datetime);
        self.download_media(&tweet, &temp_dir, &base_name).await;

        // move temp_dir to target_dir
        fs::rename(&temp_dir, &target_dir).unwrap();
    }

    fn write_tweet_text(
        &self,
        tweet: &Tweet,
        dir_path: impl AsRef<OsStr>,
        base_name: &str,
        datetime: &DateTime<FixedOffset>,
    ) {
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

        let mut data = "".to_string();
        data.push_str(&format!(
            "url: https://twitter.com/i/status/{}\n",
            &tweet.id
        ));
        data.push_str(&format!("name: {}\n", &tweet.user.name));
        data.push_str(&format!("screen_name: {}\n", &tweet.user.screen_name));
        data.push_str(&format!("created_at: {}\n", &datetime.format("%+")));
        data.push_str(&format!("\n"));
        data.push_str(&format!("{}\n", body));

        let file_name = Path::new(&dir_path).join(format!("{}.txt", &base_name));
        let f = File::create(file_name).unwrap();
        let mut f = BufWriter::new(f);
        f.write_all(data.as_bytes()).unwrap();
    }

    fn unicode_slice(&self, input: &str, bounds: [usize; 2]) -> String {
        input
            .chars()
            .skip(bounds[0])
            .take(bounds[1].checked_sub(bounds[0]).unwrap())
            .collect()
    }

    async fn download_media(&self, tweet: &Tweet, dir_path: impl AsRef<OsStr>, base_name: &str) {
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
            for (photo, i) in photos.iter().zip(1 as usize..) {
                let file_name = Path::new(&dir_path).join(format!(
                    "{}_img{}.{}",
                    &base_name,
                    i,
                    self.url_file_ext(&photo.media_url_https)
                ));
                self.download_file(&photo.media_url_https, &[("name", "orig")], &file_name)
                    .await;
            }

            // Download videos
            for (video, i) in videos.iter().zip(1 as usize..) {
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
                    self.url_file_ext(&max_video.url)
                ));
                self.download_file(&max_video.url, &[], &file_name).await;
            }
        }
    }

    async fn download_file(
        &self,
        url: &str,
        query: &[(&str, &str)],
        path: impl AsRef<Path> + AsRef<OsStr>,
    ) {
        loop {
            let resp = self.client.get(url).query(&query).send().await.unwrap();

            if resp.status().is_success() {
                let mut file = File::create(&path).unwrap();
                let mut stream = resp.bytes_stream();
                while let Some(b) = stream.next().await {
                    let chunk = b.unwrap();
                    file.write(&chunk).unwrap();
                }
                return;
            }

            if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                let rate_reset_at = resp
                    .headers()
                    .get("x-rate-limit-reset")
                    .unwrap()
                    .to_str()
                    .unwrap();
                let rate_reset_within = Duration::from_secs(rate_reset_at.parse::<u64>().unwrap())
                    - UNIX_EPOCH.elapsed().unwrap();
                eprintln!("Rate limit hit, sleeping for {:?}", rate_reset_within);
                sleep(rate_reset_within).await;
                continue;
            }

            eprintln!("{:#?}", &resp);
            panic!();
        }
    }

    fn url_file_ext(&self, url: &str) -> String {
        let parsed = Url::parse(&url).unwrap();
        let path = parsed.path();
        Path::new(&path)
            .extension()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string()
    }
}
