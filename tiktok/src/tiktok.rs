use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures::Stream;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, HOST, USER_AGENT};
use reqwest::Client;
use scraper::{Html, Selector};
use serde::Deserialize;
use sns_archive_common::{set_mtime, SavablePost};
use time::{OffsetDateTime, UtcOffset};
use tokio::{fs, process};

#[derive(Debug)]
pub struct TikTokVideo {
    id: String,
    datetime: time::OffsetDateTime,
    user: String,
    download_url: String,
}

pub struct TikTokClient<'a> {
    reqwest_client: &'a Client,
}

pub struct TikTokVideoShort {
    url: String,
}

pub struct TikTokVideos {
    all_urls: VecDeque<String>,
}

impl<'a> TikTokClient<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self {
            reqwest_client: client,
        }
    }

    pub async fn videos_from_html(&self, html: &str) -> Result<TikTokVideos> {
        let html = Html::parse_fragment(&html);
        let urls = self.extract_videos(&html).await;
        Ok(TikTokVideos::new(urls))
    }

    pub async fn latest_user_videos(&self, user: &str) -> Result<TikTokVideos> {
        let text = self
            .reqwest_client
            .get(format!("https://www.tiktok.com/@{}", user))
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        let html = Html::parse_fragment(&text);
        let urls = self.extract_videos(&html).await;
        Ok(TikTokVideos::new(urls))
    }

    async fn extract_videos(&self, html: &Html) -> Vec<String> {
        // Find posts list
        let selector = Selector::parse("div[data-e2e=\"user-post-item-list\"]").unwrap();
        let posts_list = html.select(&selector).next().unwrap();

        // Parse posts link
        let selector = Selector::parse("div[data-e2e=\"user-post-item\"] a").unwrap();
        let links = posts_list
            .select(&selector)
            .map(|e| e.value().attr("href").unwrap().to_owned())
            .collect();
        links
    }
}

#[async_trait]
impl SavablePost for TikTokVideo {
    async fn download(&self, client: &Client, directory: impl AsRef<Path> + Send) -> Result<()> {
        let filename = directory.as_ref().join(format!("{}.mp4", self.slug()?));
        let filename_temp = directory
            .as_ref()
            .join(format!("{}.mp4.temp", self.slug()?));

        let data = client.get(&self.download_url).send().await?.bytes().await?;
        fs::write(&filename_temp, data).await?;

        // Convert to mp4
        process::Command::new("ffmpeg")
            .arg("-y")
            .arg("-i")
            .arg(&filename_temp)
            .arg("-c")
            .arg("copy")
            .arg("-movflags")
            .arg("+faststart")
            .arg(&filename)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap()
            .wait()
            .await?;

        // Delete temp file
        fs::remove_file(&filename_temp).await?;

        // Set file metadata time
        set_mtime(&filename, &self.datetime)?;

        Ok(())
    }

    fn slug(&self) -> Result<String> {
        let fmt = time::format_description::parse("[year][month][day]").unwrap();
        let date_str = self.datetime.format(&fmt).unwrap();
        let slug = format!("{}_{}_{}", date_str, &self.id, &self.user);
        Ok(slug)
    }
}
async fn video_info(client: &Client, url: &str) -> Result<TikTokVideo> {
    let snaptik_token = snaptik_token(client).await?;
    for i in 0..5 {
        // Query TikTok
        let mut headers = HeaderMap::new();
        headers.insert(HOST, HeaderValue::from_static("www.tiktok.com"));
        headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
        headers.insert(USER_AGENT, HeaderValue::from_static("HTTPie/2.6.0"));
        let text = client
            .get(url)
            .headers(headers)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .text()
            .await
            .unwrap();
        let html = Html::parse_fragment(&text);
        let selector = Selector::parse(r#"script#SIGI_STATE[type="application/json"]"#).unwrap();
        let json = if let Some(j) = html.select(&selector).next() {
            j.inner_html()
        } else {
            continue;
        };

        // Get ID
        let id = url.rsplit_once('/').unwrap().1;

        // Parse date and author
        #[derive(Deserialize)]
        struct TTInfo {
            #[serde(rename = "ItemModule")]
            item_module: HashMap<String, TTVideoInfo>,
        }
        #[derive(Deserialize)]
        struct TTVideoInfo {
            #[serde(rename = "createTime")]
            create_time: String,
            author: String,
        }

        let info: TTInfo = serde_json::from_str(&json).unwrap();
        let x = match info.item_module.get(id) {
            Some(x) => x,
            None => {
                let n = 1 << i;
                eprintln!("Missing ID for {}, sleeping {} seconds", &url, n);
                tokio::time::sleep(Duration::from_secs(n)).await;
                continue;
            }
        };
        let datetime = OffsetDateTime::from_unix_timestamp(x.create_time.parse().unwrap())
            .unwrap()
            .to_offset(UtcOffset::from_hms(9, 0, 0).unwrap());

        // Get download url
        let download_url = snaptik_get_video(client, &snaptik_token, url).await?;

        return Ok(TikTokVideo {
            id: id.to_owned(),
            datetime,
            user: x.author.clone(),
            download_url,
        });
    }

    Err(anyhow!("Missing info for {}", url)).into()
}

async fn snaptik_get_video(client: &Client, token: &str, url: &str) -> Result<String> {
    // Query snaptik
    let form = reqwest::multipart::Form::new()
        .text("url", url.to_owned())
        .text("token", token.to_owned());
    let script = client
        .post("https://snaptik.app/abc2.php")
        .multipart(form)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    // Deobfuscate
    let decoded = snaptik_decode(&script).await;

    // Extract full hd video
    let re = regex::Regex::new(r#"(https:[\./\w\?&%=\-]*?)\\""#).unwrap();
    let url = re
        .captures(&decoded)
        .unwrap()
        .get(1)
        .unwrap()
        .as_str()
        .to_owned();
    Ok(url)
}

async fn snaptik_token(client: &Client) -> Result<String> {
    let text = client
        .get("https://snaptik.app/en")
        .send()
        .await?
        .text()
        .await?;
    let html = Html::parse_fragment(&text);
    let selector = Selector::parse("input[name=\"token\"]").unwrap();
    let input = html.select(&selector).next().unwrap();
    Ok(input.value().attr("value").unwrap().to_owned())
}

async fn snaptik_decode(text: &str) -> String {
    let re = regex::Regex::new(r"eval\((?P<func1>function)(?P<func2>.*})\s*\(+(?P<args>.+?)\)+")
        .unwrap();
    let script = re.replace(
        text,
        "${func1} decode_impl${func2}\nfunction decode() { return decode_impl(${args}); }",
    );

    let mut script = js_sandbox::Script::from_string(&script).unwrap();
    script.call("decode", &()).unwrap()
}

impl TikTokVideoShort {
    pub async fn video_info(&self, client: &Client) -> Result<TikTokVideo> {
        video_info(client, &self.url).await
    }
}

impl TikTokVideos {
    fn new(urls: impl IntoIterator<Item = String>) -> Self {
        Self {
            all_urls: urls.into_iter().collect(),
        }
    }

    pub async fn as_stream<'a>(&'a mut self) -> impl Stream<Item = Result<TikTokVideoShort>> + 'a {
        futures::stream::unfold(self, |state| async {
            // Pop off and return the next video if it exists
            if let Some(url) = state.all_urls.pop_front() {
                return Some((Ok(TikTokVideoShort { url }), state));
            }
            None
        })
    }
}
