use std::fmt::Display;
use std::path::Path;

use anyhow::Result;
use futures::{future, StreamExt};
use reqwest::Client;
use sns_archive_common::SavablePost;
use tokio::fs;
use weibo::{WeiboClient, WeiboPost};

use crate::config::weibo::{WeiboConfig, WeiboUserConfig};

pub async fn download(conf: WeiboConfig) -> Result<()> {
    let client = Client::new();
    let weibo_client = WeiboClient::init(&client).await?;

    let mut errored = false;

    for WeiboUserConfig { user, download_path } in conf.users {
        // Download posts
        let mut posts = weibo_client.posts(user).await?;
        let posts_stream = posts.as_stream(&client).await;
        futures::pin_mut!(posts_stream);
        fs::create_dir_all(&download_path).await?;
        posts_stream
            .map(|p| download_post(&download_path, &client, p))
            .buffered(conf.max_connections)
            .take_while(|r| {
                let ret = match r {
                    Ok(DownloadStatus::Skipped) => false,
                    Ok(DownloadStatus::Downloaded) => true,
                    Err(e) => {
                        println!("Error: {:?}", e);
                        errored = true;
                        true
                    }
                };
                future::ready(ret)
            })
            .collect::<Vec<_>>()
            .await;
    }

    if errored {
        Err(Error.into())
    } else {
        Ok(())
    }
}

#[derive(Debug)]
struct Error;

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "errors occured wile downloading Weverse posts")
    }
}

impl std::error::Error for Error {}

#[derive(Debug)]
enum DownloadStatus {
    Downloaded,
    Skipped,
}

async fn download_post(
    download_dir: impl AsRef<Path>,
    client: &Client,
    post: Result<WeiboPost>,
) -> Result<DownloadStatus> {
    let post = post?;
    let slug = post.slug()?;
    if download_dir.as_ref().join(&slug).exists() {
        return Ok(DownloadStatus::Skipped);
    }

    download_post_real(download_dir.as_ref(), client, &post).await?;
    Ok(DownloadStatus::Downloaded)
}

async fn download_post_real(
    path: impl AsRef<Path>,
    client: &Client,
    post: &WeiboPost,
) -> Result<()> {
    // Create temporary directory
    let slug = post.slug()?;
    let temp_dir = path.as_ref().join(format!(".{}.temp", post.slug()?));
    fs::create_dir_all(&temp_dir).await?;
    let final_dir = path.as_ref().join(&slug);

    // Download to temp directory
    post.download(client, &temp_dir).await?;

    // Move temp directory to final location
    fs::rename(&temp_dir, &final_dir).await?;

    println!("Downloaded {}", slug);

    Ok(())
}
