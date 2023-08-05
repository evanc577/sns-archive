use std::path::Path;
use anyhow::Result;
use reqwest::Client;
use sns_archive_common::{SavablePost, osstr_starts_with};
use tiktok::{TikTokClient, TikTokVideo};
use tokio::fs;

use crate::config::tiktok::TikTokConfig;

pub async fn download(conf: TikTokConfig) -> Result<()> {
    let client = Client::new();
    let tt_client = TikTokClient::new(&client);
    for (user, user_config) in conf.users {
        println!("Downloading {} videos", &user);
        let videos = tt_client.latest_user_videos(&user).await?;
        for tt_video in videos {
            match download_video(&user_config.download_path, &client, tt_video).await? {
                DownloadStatus::Downloaded => (),
                DownloadStatus::Skipped => break,
            }
        }
    }
    Ok(())
}

pub async fn download_from_html(input_file: impl AsRef<Path>) -> Result<()> {
    let client = Client::new();
    let tt_client = TikTokClient::new(&client);
    let html = fs::read_to_string(input_file).await?;
    let videos = tt_client.videos_from_html(&html).await?;
    for tt_video in videos {
        match download_video(std::env::current_dir()?, &client, tt_video).await? {
            DownloadStatus::Downloaded => (),
            DownloadStatus::Skipped => break,
        }
    }
    Ok(())
}

#[derive(Debug)]
enum DownloadStatus {
    Downloaded,
    Skipped,
}

async fn download_video(
    download_dir: impl AsRef<Path>,
    client: &Client,
    video: TikTokVideo,
) -> Result<DownloadStatus> {
    let slug = video.slug()?;
    fs::create_dir_all(&download_dir).await?;
    let mut read_dir = fs::read_dir(download_dir.as_ref()).await?;
    while let Some(f) = read_dir.next_entry().await? {
        if osstr_starts_with(&f.file_name(), &slug) {
            return Ok(DownloadStatus::Skipped);
        }
    }

    // Create temporary directory
    let temp_dir = download_dir
        .as_ref()
        .join(format!(".{}.temp", video.slug()?));
    fs::create_dir_all(&temp_dir).await?;

    // Download to temporary directory
    video.download(client, &temp_dir).await?;

    // Move files out of temporary directory
    let mut read_dir = fs::read_dir(&temp_dir).await?;
    while let Some(f) = read_dir.next_entry().await? {
        fs::rename(f.path(), &download_dir.as_ref().join(f.file_name())).await?;
    }
    fs::remove_dir(temp_dir).await?;

    println!("Downloaded {}", slug);

    Ok(DownloadStatus::Downloaded)
}
