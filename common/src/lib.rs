use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use futures::stream::StreamExt;
use reqwest::{Client, IntoUrl};
use tokio::fs;
use tokio::io::AsyncWriteExt;

#[async_trait]
pub trait SavablePost {
    async fn download(&self, client: &Client, directory: impl AsRef<Path> + Send) -> Result<()>;
    fn slug(&self) -> Result<String>;
}

pub async fn streamed_download(
    client: &Client,
    url: impl IntoUrl,
    path: impl AsRef<Path>,
) -> Result<()> {
    let resp = client.get(url.as_str()).send().await?.error_for_status()?;
    let mut file = fs::File::create(path).await?;
    let mut stream = resp.bytes_stream();
    while let Some(b) = stream.next().await {
        let chunk = b?;
        file.write_all(&chunk).await?;
    }

    Ok(())
}
