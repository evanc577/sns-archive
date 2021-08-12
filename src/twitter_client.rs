use crate::Config;

use anyhow::{Context, Result};
use itertools::Itertools;
use reqwest::{header::*, Client, ClientBuilder};
use serde::{Deserialize, Deserializer};
use std::cmp::Ordering;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{prelude::*, BufReader};
use std::path::Path;
use tokio::time::Duration;

pub struct TwitterClient<'a> {
    client: Client,
    config: &'a Config,
}

#[derive(Deserialize, Debug)]
pub struct Tweet {
    pub created_at: String,
    pub id: u64,
    pub id_str: String,
    pub full_text: String,
    pub truncated: bool,
    pub display_text_range: Vec<u64>,
    pub entities: Entities,
    pub extended_entities: Option<ExtendedEntities>,
    pub user: User,
    #[serde(deserialize_with = "deserialize_retweet_status")]
    #[serde(default)]
    pub retweeted_status: bool,
}

#[derive(Deserialize, Debug)]
pub struct Entities {
    pub hashtags: Vec<HashtagEntity>,
    pub urls: Vec<UrlEntity>,
}

#[derive(Deserialize, Debug)]
pub struct HashtagEntity {
    pub indices: Vec<u64>,
    pub text: String,
}

#[derive(Deserialize, Debug)]
pub struct UrlEntity {
    pub display_url: String,
    pub expanded_url: String,
    pub indices: Vec<u64>,
    pub url: String,
}

#[derive(Deserialize, Debug)]
pub struct ExtendedEntities {
    pub media: Vec<Media>,
}

#[derive(Deserialize, Debug)]
pub struct Media {
    pub expanded_url: String,
    pub media_url: String,
    pub media_url_https: String,
    #[serde(rename = "type")]
    pub media_type: String,
    pub url: String,
    pub video_info: Option<VideoInfo>,
}

#[derive(Deserialize, Debug)]
pub struct VideoInfo {
    pub aspect_ratio: Vec<u64>,
    pub duration_millis: Option<u64>,
    pub variants: Vec<VideoVariants>,
}

#[derive(Deserialize, Debug)]
pub struct VideoVariants {
    pub bitrate: Option<u64>,
    pub content_type: String,
    pub url: String,
}

#[derive(Deserialize, Debug)]
pub struct User {
    pub created_at: String,
    pub description: String,
    pub favourites_count: u64,
    pub followers_count: u64,
    pub friends_count: u64,
    pub has_extended_profile: bool,
    pub id: u64,
    pub id_str: String,
    pub listed_count: u64,
    pub name: String,
    pub screen_name: String,
    pub verified: bool,
}

fn deserialize_retweet_status<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let s: serde_json::Value = Deserialize::deserialize(deserializer)?;
    Ok(!s.is_null())
}

impl PartialEq for Tweet {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl PartialOrd for Tweet {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.id.cmp(&other.id))
    }
}

impl Eq for Tweet {}

impl Ord for Tweet {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

impl<'a> TwitterClient<'a> {
    pub fn new(config: &'a Config) -> TwitterClient {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            format!("Bearer {}", &config.bearer).parse().unwrap(),
        );

        let client = ClientBuilder::new()
            .default_headers(headers)
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap();

        TwitterClient { client, config }
    }

    pub async fn process_user(&self, user: &str) -> Result<Vec<Tweet>>{
        let path = Path::new(&self.config.directory).join(user);
        let start = match std::fs::read_dir(path) {
            Err(_) => None,
            Ok(i) => i
                .filter_map(|n| {
                    let max_id = n
                        .ok()?
                        .file_name()
                        .to_str()?
                        .to_string()
                        .split('_')
                        .skip(1)
                        .next()?
                        .parse::<u64>()
                        .ok()?;
                    Some(max_id)
                })
                .max(),
        };

        let tweets = self.user_timeline(user, start).await?;
        Ok(tweets)
    }

    pub async fn process_ids_file(
        &self,
        path: impl AsRef<Path> + AsRef<OsStr>,
    ) -> Result<Vec<Tweet>> {
        let file = File::open(&path)?;
        let reader = BufReader::new(file);

        let ids: Vec<_> = reader
            .lines()
            .filter_map(|l| Some(l.ok()?.trim().to_string()))
            .collect();

        let tweets = self.lookup(ids.iter().map(|s| s.as_ref())).await?;

        Ok(tweets)
    }

    pub async fn lookup(&self, tweet_ids: impl Iterator<Item = &str>) -> Result<Vec<Tweet>> {
        let chunks = tweet_ids.chunks(100);

        let mut all_tweets = vec![];

        eprintln!("Querying tweets...");
        for chunk in chunks.into_iter() {
            let id: String = chunk.intersperse(",").collect();
            let params = [
                ("id", id.as_str()),
                ("include_entities", "true"),
                ("trim_user", "false"),
                ("tweet_mode", "extended"),
            ];

            let resp = self
                .client
                .post("https://api.twitter.com/1.1/statuses/lookup.json")
                .form(&params)
                .send()
                .await?;

            let tweets: Vec<Tweet> = resp.json().await.context("Failed to parse lookup endpoint JSON")?;
            all_tweets.extend(tweets);
        }

        Ok(all_tweets
            .into_iter()
            .filter(|t| !t.retweeted_status)
            .collect())
    }

    pub async fn user_timeline(&self, screen_name: &str, since_id: Option<u64>) -> Result<Vec<Tweet>> {
        let mut params_base = vec![
            ("screen_name", screen_name.to_owned()),
            ("count", "200".to_string()),
            ("include_rts", "true".to_string()),
            ("exclude_replies", "false".to_string()),
            ("include_entities", "true".to_string()),
            ("trim_user", "false".to_string()),
            ("tweet_mode", "extended".to_string()),
        ];
        if let Some(id) = since_id {
            params_base.push(("since_id", id.to_string()));
        }

        let mut all_tweets = vec![];

        let mut max_id: Option<u64> = None;
        eprintln!("Retrieving {}...", &screen_name);
        loop {
            // Query Twitter API
            let mut params = params_base.clone();
            if let Some(id) = max_id {
                params.push(("max_id", id.to_string()));
            }

            let resp = self
                .client
                .get("https://api.twitter.com/1.1/statuses/user_timeline.json")
                .form(&params)
                .send()
                .await?;

            // Exit loop if no more tweets
            let tweets: Vec<Tweet> = resp.json().await.context("Failed to parse user_timeline endpoint JSON")?;
            if tweets.is_empty() {
                break;
            }

            // Update max_id for next iteration
            let min_id = tweets.iter().min().unwrap().id;
            if let Some(max_id) = max_id {
                if min_id == max_id {
                    break;
                }
            }
            if let Some(since_id) = since_id {
                if since_id >= min_id {
                    break;
                }
            }
            max_id = Some(min_id - 1);

            // Append new tweets
            all_tweets.extend(tweets);
        }

        Ok(all_tweets
            .into_iter()
            .filter(|t| t.user.screen_name == screen_name)
            .filter(|t| !t.retweeted_status)
            .collect())
    }
}
