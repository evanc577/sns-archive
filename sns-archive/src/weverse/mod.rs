use anyhow::Result;
use futures::{future, StreamExt};
use reqwest::Client;
use sns_archive_common::SavablePost;
use tokio::fs;
use weverse::endpoint::artist_tab_posts::ArtistPostShort;
use weverse::{AuthenticatedWeverseClient, LoginInfo};

use crate::config::weverse::{ArtistConfig, WeverseConfig};

pub async fn download(conf: WeverseConfig) -> Result<()> {
    let client = Client::new();
    let login_info = LoginInfo {
        email: conf.email,
        password: conf.password,
    };
    let weverse_client = AuthenticatedWeverseClient::login(&client, &login_info).await?;

    for (artist, artist_config) in conf.artists {
        let mut posts = weverse_client.artist_posts(&artist).await?;
        let posts_stream = posts.as_stream(&client).await;
        futures::pin_mut!(posts_stream);

        fs::create_dir_all(&artist_config.artist_download_path).await?;
        fs::create_dir_all(&artist_config.moments_download_path).await?;

        posts_stream
            .map(|p| download_post(&artist_config, &client, &weverse_client, p))
            .buffer_unordered(conf.max_connections)
            .filter_map(|r| {
                let ret = match r {
                    Ok(_) => None,
                    e @ Err(_) => Some(e),
                };
                future::ready(ret)
            })
            .for_each(|e| {
                println!("Error: {:?}", e);
                future::ready(())
            })
            .await;
    }

    Ok(())
}

async fn download_post(
    artist_config: &ArtistConfig,
    client: &Client,
    weverse_client: &AuthenticatedWeverseClient<'_>,
    post: Result<ArtistPostShort>,
) -> Result<()> {
    let post = post?;
    let slug = post.slug()?;
    if artist_config.artist_download_path.join(&slug).exists() {
        println!("Skipping {}", slug);
        return Ok(());
    }
    let post = weverse_client.post(&post.post_id).await?;

    // Create temporary directory
    let slug = post.slug()?;
    let temp_dir = artist_config
        .artist_download_path
        .join(format!(".{}.temp", post.slug()?));
    fs::create_dir_all(&temp_dir).await?;
    let final_dir = artist_config.artist_download_path.join(&slug);

    // Download to temp directory
    post.download(client, &temp_dir).await?;

    // Move temp directory to final location
    fs::rename(&temp_dir, &final_dir).await?;

    println!("Downloaded {}", slug);

    Ok(())
}
