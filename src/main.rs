use crate::config::Config;
use crate::downloader::DownloadClient;
use crate::twitter_client::{Tweet, TwitterClient};

use clap::{App, Arg};

mod config;
mod downloader;
mod twitter_client;

#[tokio::main]
async fn main() {
    let matches = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(
            Arg::with_name("INPUT")
                .short("i")
                .long("input")
                .value_name("FILE")
                .help("Read tweets to save from input file, 1 tweet ID per line")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("FILTER")
                .short("f")
                .long("filter")
                .value_name("SCREEN_NAME")
                .help("Only save tweets from user")
                .takes_value(true),
        )
        .get_matches();
    let config = Config::read();

    // Retrieve tweets
    let twitter_client = TwitterClient::new(&config);
    let mut all_tweets: Vec<Tweet> = vec![];
    if let Some(input_file) = matches.value_of("INPUT") {
        let tweets = twitter_client.process_ids_file(&input_file).await.unwrap();
        all_tweets.extend(tweets);
    } else {
        for user in &config.users {
            let tweets = twitter_client.process_user(&user).await.unwrap();
            all_tweets.extend(tweets);
        }
    }

    // Download tweets
    let download_client = DownloadClient::new(&config);
    download_client
        .download_tweets(all_tweets.iter(), matches.value_of("FILTER"))
        .await;
}
