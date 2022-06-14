use std::path::Path;

use anyhow::Result;

use self::downloader::DownloadClient;
use self::twitter_client::TwitterClient;
use crate::config::twitter::TwitterConfig;

mod downloader;
mod response_helpers;
mod tweet;
mod twitter_client;

pub async fn download(
    config: TwitterConfig,
    input_file: Option<&Path>,
    filter: Option<&str>,
) -> Result<()> {
    // Retrieve tweets
    let twitter_client = TwitterClient::new(&config);
    let all_tweets = if let Some(input_file) = input_file {
        twitter_client.process_ids_file(input_file).await?
    } else {
        twitter_client
            .process_users(config.users.iter().map(|s| s.as_ref()))
            .await?
    };

    // Download tweets
    let download_client = DownloadClient::new(&config);
    download_client
        .download_tweets(all_tweets.iter(), filter)
        .await;

    Ok(())
}
