use std::ffi::OsStr;
use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use futures::stream::StreamExt;
use reqwest::{Client, IntoUrl};
use time::OffsetDateTime;
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
    // Download file
    let resp = client.get(url.as_str()).send().await?.error_for_status()?;
    let mut file = fs::File::create(&path).await?;
    let mut stream = resp.bytes_stream();
    while let Some(b) = stream.next().await {
        let chunk = b?;
        file.write_all(&chunk).await?;
    }

    Ok(())
}

pub fn set_mtime(path: impl AsRef<Path>, mtime: &OffsetDateTime) -> Result<()> {
    let mtime_unix = mtime.unix_timestamp_nanos();
    const ONE_BILLION: i128 = 1_000_000_000;
    let secs = (mtime_unix / ONE_BILLION) as i64;
    let nanos = (mtime_unix % ONE_BILLION) as u32;
    let mtime = filetime::FileTime::from_unix_time(secs, nanos);
    filetime::set_file_mtime(&path, mtime)?;
    Ok(())
}

pub fn osstr_starts_with(osstr: &OsStr, start: &str) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::prelude::OsStrExt;
        osstr.as_bytes().starts_with(start.as_bytes())
    }
    #[cfg(windows)]
    {
        use std::ffi::OsString;
        use std::str::FromStr;
        use std::os::windows::prelude::OsStrExt;
        let osstr_vec: Vec<u16> = osstr.encode_wide().collect();
        let start_vec: Vec<u16> = OsString::from_str(start).unwrap().as_os_str().encode_wide().collect();
        osstr_vec.starts_with(&start_vec[..])
    }
}
