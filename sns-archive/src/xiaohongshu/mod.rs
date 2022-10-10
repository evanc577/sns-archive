use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Deserializer};
use thirtyfour::prelude::*;
use thirtyfour::CapabilitiesHelper;
use tokio::io::AsyncWriteExt;
use tokio::{fs, process, time};
use unicode_segmentation::UnicodeSegmentation;

use crate::config::xiaohongshu::XiaoHongShuConfig;

static DRIVER_ADDR: &str = "http://localhost:4444";

#[derive(Deserialize, Debug)]
struct XHSResponse {
    data: XHSData,
}

#[derive(Deserialize, Debug)]
struct XHSData {
    notes: Vec<XHSNote>,
}

#[derive(Deserialize, Debug)]
struct XHSNote {
    id: String,
    display_title: String,
    desc: String,
    images_list: Vec<XHSImage>,
    video_info: Option<XHSVideo>,
    user: XHSUser,
}

#[derive(Deserialize, Debug)]
struct XHSImage {
    #[serde(rename = "url_size_large")]
    #[serde(deserialize_with = "deserialize_url_remove_query")]
    url: String,
}

#[derive(Deserialize, Debug)]
struct XHSVideo {
    #[serde(rename = "adaptive_streaming_url_set")]
    adaptive_videos: Option<Vec<XHSAdaptiveVideo>>,
    url: Option<String>,
}

#[derive(Deserialize, Debug)]
struct XHSAdaptiveVideo {
    avg_bitrate: u64,
    url: String,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct XHSUser {
    nickname: String,
    userid: String,
    #[serde(rename = "images")]
    #[serde(deserialize_with = "deserialize_url_remove_query")]
    image: String,
}

fn deserialize_url_remove_query<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    let s = match s.split_once('?') {
        Some((s, _)) => s.to_owned(),
        None => s,
    };
    Ok(s)
}

pub async fn download(json_file: impl AsRef<Path>, config: XiaoHongShuConfig) -> Result<()> {
    // Parse JSON
    let s = fs::read_to_string(json_file.as_ref()).await?;
    let parsed: XHSResponse = serde_json::from_str(&s)?;

    // Create directory
    fs::create_dir_all(&config.download_path).await?;

    // Create selenium driver
    let _driver = process::Command::new("geckodriver")
        .arg("--port=4444")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let mut caps = DesiredCapabilities::firefox();
    caps.set("pageLoadStrategy".into(), serde_json::json!("none"));
    let driver = loop {
        match WebDriver::new(DRIVER_ADDR, caps.clone()).await {
            Ok(d) => break d,
            _ => time::sleep(Duration::from_secs(1)).await,
        }
        println!("Waiting for driver...");
    };

    // Download posts
    for post in parsed.data.notes {
        download_post(&driver, post, &config.download_path).await?;
    }

    // Close window
    driver.close().await?;
    driver.quit().await?;

    Ok(())
}

async fn download_post(driver: &WebDriver, post: XHSNote, dir: impl AsRef<Path>) -> Result<()> {
    // Create user directory
    let user_dir = dir.as_ref().join(&post.user.nickname);
    fs::create_dir_all(&user_dir).await?;

    // Check if already downloaded
    for direntry in std::fs::read_dir(&user_dir)?.flatten() {
        if direntry.file_name().to_string_lossy().contains(&post.id) {
            return Ok(());
        }
    }

    // Create post directory
    let prefix = {
        let date = match get_date(driver, &post.id).await {
            Ok(d) => d,
            _ => {
                // Query user for date
                println!(
                    "Enter date for {} - {} - {}",
                    &post.id, &post.user.nickname, &post.display_title
                );
                let mut date_buffer = String::new();
                let stdin = std::io::stdin();
                stdin.read_line(&mut date_buffer)?;
                date_buffer
            }
        };
        let x = sanitize_filename::sanitize(format!(
            "{}-{}-{}-{}",
            &date, &post.user.nickname, &post.id, &post.display_title
        ));
        let truncated: String = UnicodeSegmentation::grapheme_indices(x.as_str(), true)
            .filter_map(|(i, x)| if i < 150 { Some(x) } else { None })
            .collect();
        truncated
    };
    let post_dir = user_dir.join(&prefix);
    fs::create_dir_all(&post_dir).await?;

    // Download images
    for (i, image) in post.images_list.into_iter().enumerate() {
        let filename = format!("{}-img{:02}", &prefix, i + 1);
        let path = post_dir.join(&filename);
        download_file(image.url, path).await?;
    }

    // Download video
    if let Some(v) = post.video_info {
        let url = if let Some(v) = v.adaptive_videos {
            v.into_iter()
                .max_by_key(|x| x.avg_bitrate)
                .ok_or_else(|| anyhow::anyhow!("No videos found"))?
                .url
        } else if let Some(u) = v.url {
            u
        } else {
            panic!("No video available");
        };
        let filename = format!("{}-vid", &prefix);
        let path = post_dir.join(&filename);
        download_file(url, path).await?;
    }

    // Write content file
    {
        let link = format!(
            "https://www.xiaohongshu.com/discovery/item/{}?xhsshare=CopyLink",
            post.id
        );
        let file_contents = format!("{}\n{}\n\n{}", link, post.user.nickname, post.desc);
        let filename = format!("{}-content.txt", &prefix);
        let path = post_dir.join(&filename);
        let mut file = fs::File::create(&path).await?;
        file.write_all(file_contents.as_bytes()).await?;
    }

    println!("Downloaded {}", prefix);

    Ok(())
}

async fn download_file(url: impl AsRef<str>, path: impl AsRef<Path>) -> Result<()> {
    let data = reqwest::get(url.as_ref()).await?.bytes().await?;
    let kind = infer::get(&data).ok_or_else(|| anyhow::anyhow!("Unknown file type"))?;
    let mut file = fs::File::create(path.as_ref().with_extension(kind.extension())).await?;
    file.write_all(&data).await?;

    Ok(())
}

async fn get_date(driver: &WebDriver, id: &str) -> Result<String> {
    // Visit webpage
    driver.get("about:blank").await?;
    driver
        .get(format!(
            "https://www.xiaohongshu.com/discovery/item/{}?xhsshare=CopyLink",
            id
        ))
        .await?;

    // Wait until date loads
    let date = loop {
        if driver.current_url().await?.as_str().contains("captcha") {
            return Err(anyhow::anyhow!("Captcha page"));
        }
        match driver.find_element(By::ClassName("publish-date")).await {
            Ok(e) => break e.text().await?,
            _ => time::sleep(Duration::from_secs(1)).await,
        }
    };

    // Parse date
    let re = regex::Regex::new(r"\d{4}-\d{2}-\d{2}")?;
    let m = re
        .find(&date)
        .ok_or_else(|| anyhow::anyhow!("Could not parse date {}", &date))?;
    let date_str = m.as_str().replace('-', "").split_off(2);

    Ok(date_str)
}
