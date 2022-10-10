use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use futures::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use lazy_static::lazy_static;
use regex::Regex;
use reqwest::Client;
use time::format_description;
use unicode_segmentation::UnicodeSegmentation;
use weverse::endpoint::vod::{vod_info, VodInfo};

#[derive(Parser)]
struct Args {
    url: String,

    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,
}

lazy_static! {
    static ref URL_RE: Regex =
        Regex::new(r"https?://weverse.io/\w+/live/(?P<post_id>[\d\-]+)").unwrap();
}

#[derive(Debug)]
pub enum Error {
    InvalidUrl,
    NoVods,
    NoSize,
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidUrl => write!(f, "invalid URL"),
            Self::NoVods => write!(f, "no videos found"),
            Self::NoSize => write!(f, "no content size"),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Parse url
    let post_id = URL_RE
        .captures(&args.url)
        .ok_or(Error::InvalidUrl)?
        .name("post_id")
        .ok_or(Error::InvalidUrl)?
        .as_str();

    // Fetch VOD info
    let client = Client::new();
    let info = vod_info(&client, post_id).await?;

    // Create output file
    let output = match args.output {
        Some(p) => p,
        None => gen_file_name(&info),
    };
    let mut file = File::create(output)?;

    // Select best quality
    let video = info.videos.into_iter().max().ok_or(Error::NoVods)?;
    let resp = client.get(&video.source).send().await?;
    let total_size = resp.content_length().ok_or(Error::NoSize)?;

    // Initialize progress bar
    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")?
        .progress_chars("#>-"));
    pb.set_message(format!("Downloading {}", info.title));
    let mut downloaded = 0;

    // Download file
    let mut stream = resp.bytes_stream();
    while let Some(b) = stream.next().await {
        let chunk = b?;
        file.write_all(&chunk)?;
        downloaded = std::cmp::min(downloaded + chunk.len() as u64, total_size);
        pb.set_position(downloaded);
    }

    Ok(())
}

fn gen_file_name(vod_info: &VodInfo) -> PathBuf {
    // Format date
    let format = format_description::parse("[year][month][day]").unwrap();
    let date_str = vod_info.time.format(&format).unwrap();

    // Format title
    let title_trunc: String = UnicodeSegmentation::graphemes(vod_info.title.as_str(), true)
        .filter(|c| *c != "/")
        .take(150)
        .collect();

    // Format extension
    let ext = vod_info
        .url
        .path()
        .rsplit_once('.')
        .map(|(_, ext)| ext)
        .unwrap_or("mp4");

    PathBuf::from(format!(
        "{}-{}-{}.{}",
        date_str, vod_info.id, title_trunc, ext
    ))
}
