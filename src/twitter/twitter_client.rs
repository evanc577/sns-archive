use std::ffi::OsStr;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::Path;

use anyhow::{Context, Result};
use futures::stream::StreamExt;
use itertools::Itertools;
use reqwest::header::*;
use reqwest::{Client, ClientBuilder};
use tokio::time::{sleep, Duration};

use super::response_helpers;
use super::tweet::Tweet;
use crate::config::twitter::TwitterConfig;

#[derive(Debug)]
struct TwitterError {
    text: String,
}

impl std::fmt::Display for TwitterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.text)
    }
}

impl std::error::Error for TwitterError {}

impl TwitterError {
    fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
        }
    }
}

pub struct TwitterClient<'a> {
    client: Client,
    config: &'a TwitterConfig,
}

impl<'a> TwitterClient<'a> {
    pub fn new(config: &'a TwitterConfig) -> TwitterClient {
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

        self.process_ids(ids.iter().map(|s| s.as_ref())).await
    }

    pub async fn process_ids(&self, ids: impl Iterator<Item = &str>) -> Result<Vec<Tweet>> {
        let tweets = self.lookup(ids).await?;

        Ok(tweets)
    }

    async fn lookup(&self, tweet_ids: impl Iterator<Item = &str>) -> Result<Vec<Tweet>> {
        let chunks = tweet_ids.chunks(100);

        let mut all_tweets = vec![];

        println!("Querying tweets...");
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

            if let Some(duration) = response_helpers::check_rate_limit(&resp) {
                eprintln!("Rate limit hit, sleeping for {:?}", duration);
                sleep(duration).await;
                continue;
            }

            let tweets: Vec<Tweet> = resp
                .json()
                .await
                .context("Failed to parse lookup endpoint JSON")?;
            all_tweets.extend(tweets);
        }

        Ok(all_tweets
            .into_iter()
            .unique_by(|t| t.id)
            .filter(|t| !t.retweeted_status)
            .collect())
    }

    pub async fn process_users(&self, users: impl Iterator<Item = &str>) -> Result<Vec<Tweet>> {
        let results = futures::stream::iter(users.map(|u| self.process_user(u)))
            .buffer_unordered(20)
            .collect::<Vec<_>>()
            .await;

        let (ok, err): (Vec<_>, Vec<_>) = results.into_iter().partition(|r| r.is_ok());
        if !err.is_empty() {
            for e in err {
                eprintln!("{:?}", e);
            }
            return Err(TwitterError::new("Error processing users").into());
        }

        let tweets = ok.into_iter().flatten().flatten().collect();

        Ok(tweets)
    }

    pub async fn process_user(&self, user: &str) -> Result<Vec<Tweet>> {
        let path = Path::new(&self.config.download_path).join(user);
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
                        .nth(1)?
                        .parse::<u64>()
                        .ok()?;
                    Some(max_id)
                })
                .max(),
        };

        let tweets = self.user_timeline(user, start).await?;
        Ok(tweets)
    }

    pub async fn user_timeline(
        &self,
        screen_name: &str,
        since_id: Option<u64>,
    ) -> Result<Vec<Tweet>> {
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
        println!("Retrieving {}...", &screen_name);
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

            if let Some(duration) = response_helpers::check_rate_limit(&resp) {
                eprintln!("Rate limit hit, sleeping for {:?}", duration);
                sleep(duration).await;
                continue;
            }

            // Exit loop if no more tweets
            let tweets: Vec<Tweet> = resp
                .json()
                .await
                .context("Failed to parse user_timeline endpoint JSON")?;
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
            .unique_by(|t| t.id)
            .filter(|t| t.user.screen_name.to_lowercase() == screen_name.to_lowercase())
            .filter(|t| !t.retweeted_status)
            .collect())
    }
}
