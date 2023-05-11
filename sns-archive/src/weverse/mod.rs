use std::fmt::Display;
use std::os::unix::prelude::OsStrExt;
use std::path::Path;

use anyhow::Result;
use futures::{future, stream, StreamExt};
use reqwest::Client;
use sns_archive_common::SavablePost;
use tokio::fs;
use weverse::endpoint::artist_tab_posts::ArtistPostShort;
use weverse::endpoint::post::ArtistPost;
use weverse::{AuthenticatedWeverseClient, LoginInfo};

use crate::config::weverse::WeverseConfig;

pub async fn download(conf: WeverseConfig) -> Result<()> {
    let client = Client::new();
    let login_info = LoginInfo {
        email: conf.email,
        password: conf.password,
    };
    let weverse_client = AuthenticatedWeverseClient::login(&client, &login_info).await?;

    let mut errored = false;

    for (artist, artist_config) in conf.artists {
        // Download posts
        if let Some(artist_download_path) = &artist_config.artist_download_path {
            println!("Downloading {} posts", artist);
            let mut posts = weverse_client
                .artist_posts(
                    &artist,
                    artist_config.artist_stop_id,
                    artist_config.artist_download_limit,
                )
                .await?;
            let posts_stream = posts.as_stream(&client).await;
            futures::pin_mut!(posts_stream);
            fs::create_dir_all(artist_download_path).await?;
            posts_stream
                .map(|p| download_post(artist_download_path, &client, &weverse_client, p))
                .buffered(conf.max_connections)
                .take_while(|r| {
                    let ret = match r {
                        Ok(DownloadStatus::Skipped) => {
                            artist_config.artist_download_limit.is_some()
                        }
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

        // Download moments
        if let Some(moments_download_path) = &artist_config.moments_download_path {
            println!("Downloading {} moments", artist);
            let latest_moments = weverse_client.artist_moments(&artist).await?;
            fs::create_dir_all(moments_download_path).await?;
            stream::iter(latest_moments.iter())
                .map(|p| {
                    download_member_moments(
                        moments_download_path,
                        &client,
                        &weverse_client,
                        p.clone(),
                    )
                })
                .buffer_unordered(conf.max_connections)
                .collect::<Vec<_>>()
                .await
                .into_iter()
                .filter_map(|r| r.err())
                .for_each(|e| {
                    println!("Error: {:?}", e);
                    errored = true;
                });
        }

        // Download lives
        if let Some(lives_download_path) = &artist_config.lives_download_path {
            println!("Downloading {} lives", artist);
            let mut posts = weverse_client
                .lives(
                    &artist,
                    artist_config.lives_stop_id,
                    artist_config.lives_download_limit,
                )
                .await?;
            let posts_stream = posts.as_stream(&client).await;
            futures::pin_mut!(posts_stream);
            fs::create_dir_all(lives_download_path).await?;
            posts_stream
                .map(|p| download_live(lives_download_path, &client, &weverse_client, p))
                .buffered(conf.max_connections)
                .take_while(|r| {
                    let ret = match r {
                        Ok(DownloadStatus::Skipped) => artist_config.lives_download_limit.is_some(),
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

async fn download_live(
    download_dir: impl AsRef<Path>,
    client: &Client,
    weverse_client: &AuthenticatedWeverseClient<'_>,
    post: Result<ArtistPostShort>,
) -> Result<DownloadStatus> {
    let post = post?;
    let slug = post.slug()?;
    let mut read_dir = fs::read_dir(download_dir.as_ref()).await?;
    while let Some(f) = read_dir.next_entry().await? {
        if f.file_name().as_bytes().starts_with(slug.as_bytes()) {
            return Ok(DownloadStatus::Skipped);
        }
    }

    // Create temporary directory
    let temp_dir = download_dir
        .as_ref()
        .join(format!(".{}.temp", post.slug()?));
    fs::create_dir_all(&temp_dir).await?;

    // Download to temporary directory
    let post = weverse_client.vod_info(&post.post_id).await?;
    post.download(client, &temp_dir).await?;

    // Move files out of temporary directory
    let mut read_dir = fs::read_dir(&temp_dir).await?;
    while let Some(f) = read_dir.next_entry().await? {
        fs::rename(f.path(), &download_dir.as_ref().join(f.file_name())).await?;
    }
    fs::remove_dir(temp_dir).await?;

    println!("Downloaded {}", slug);

    Ok(DownloadStatus::Downloaded)
}

async fn download_post(
    download_dir: impl AsRef<Path>,
    client: &Client,
    weverse_client: &AuthenticatedWeverseClient<'_>,
    post: Result<ArtistPostShort>,
) -> Result<DownloadStatus> {
    let post = post?;
    let slug = post.slug()?;
    if download_dir.as_ref().join(&slug).exists() {
        return Ok(DownloadStatus::Skipped);
    }

    let post = weverse_client.post(&post.post_id).await?;
    download_post_real(download_dir.as_ref(), client, &post).await?;
    Ok(DownloadStatus::Downloaded)
}

async fn download_member_moments(
    download_dir: impl AsRef<Path>,
    client: &Client,
    weverse_client: &AuthenticatedWeverseClient<'_>,
    first_post: ArtistPost,
) -> Result<()> {
    let mut post = first_post;
    loop {
        let slug = post.slug()?;
        if download_dir.as_ref().join(&slug).exists() {
            break;
        }
        download_post_real(download_dir.as_ref(), client, &post).await?;
        if let Some(next_post_id) = post.next_moment_id() {
            post = weverse_client.post(&next_post_id).await?;
        } else {
            break;
        }
    }
    Ok(())
}

async fn download_post_real(
    path: impl AsRef<Path>,
    client: &Client,
    post: &ArtistPost,
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
