use crate::config::Config;
use crate::downloader::DownloadClient;
use crate::twitter_client::TwitterClient;

use anyhow::Result;
use clap::{App, Arg};

mod config;
mod downloader;
mod response_helpers;
mod tweet;
mod twitter_client;

#[tokio::main]
async fn main() {
    let r = run().await;

    let code = match r {
        Ok(_) => exitcode::OK,
        Err(e) => {
            for cause in e.chain() {
                eprintln!("{}", cause);
            }
            exitcode::SOFTWARE
        }
    };

    std::process::exit(code);
}

async fn run() -> Result<()> {
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
    let config = Config::read()?;

    // Retrieve tweets
    let twitter_client = TwitterClient::new(&config);
    let all_tweets = if let Some(input_file) = matches.value_of("INPUT") {
        twitter_client.process_ids_file(&input_file).await?
    } else {
        twitter_client
            .process_users(config.users.iter().map(|s| s.as_ref()))
            .await?
    };

    // Download tweets
    let download_client = DownloadClient::new(&config);
    download_client
        .download_tweets(all_tweets.iter(), matches.value_of("FILTER"))
        .await;

    Ok(())
}
