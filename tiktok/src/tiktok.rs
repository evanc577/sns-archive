use std::collections::BTreeMap;
use std::path::Path;
use std::process::Stdio;

use anyhow::Result;
use async_trait::async_trait;
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
}

pub struct TikTokClient<'a> {
    reqwest_client: &'a Client,
}

impl<'a> TikTokClient<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self {
            reqwest_client: client,
        }
    }

    pub async fn videos_from_html(&self, html: &str) -> Result<Vec<TikTokVideo>> {
        let html = Html::parse_fragment(html);
        self.extract_videos(&html)
    }

    pub async fn latest_user_videos(&self, user: &str) -> Result<Vec<TikTokVideo>> {
        let text = self
            .reqwest_client
            .get(format!("https://www.tiktok.com/@{}", user))
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        let html = Html::parse_fragment(&text);
        self.extract_videos(&html)
    }

    fn extract_videos(&self, html: &Html) -> Result<Vec<TikTokVideo>> {
        let selector = Selector::parse(r#"script#SIGI_STATE[type="application/json"]"#).unwrap();
        let json = html.select(&selector).next().unwrap().inner_html();

        // Parse date and author
        #[derive(Deserialize, Debug)]
        struct TTInfo {
            #[serde(rename = "ItemModule")]
            item_module: BTreeMap<String, TTVideoInfo>,
        }
        #[derive(Deserialize, Debug)]
        struct TTVideoInfo {
            #[serde(rename = "createTime")]
            create_time: String,
            author: String,
        }

        let info: TTInfo = serde_json::from_str(&json).unwrap();
        let videos = info
            .item_module
            .into_iter()
            .rev()
            .map(|(id, video_info)| {
                let datetime =
                    OffsetDateTime::from_unix_timestamp(video_info.create_time.parse().unwrap())
                        .unwrap()
                        .to_offset(UtcOffset::from_hms(9, 0, 0).unwrap());
                let user = video_info.author;
                TikTokVideo { id, datetime, user }
            })
            .collect();

        Ok(videos)
    }
}

impl TikTokVideo {
    fn url(&self) -> String {
        format!("https://www.tiktok.com/@{}/video/{}", self.user, self.id)
    }
}

#[async_trait]
impl SavablePost for TikTokVideo {
    async fn download(&self, client: &Client, directory: impl AsRef<Path> + Send) -> Result<()> {
        let filename = directory.as_ref().join(format!("{}.mp4", self.slug()?));
        let filename_temp = directory
            .as_ref()
            .join(format!("{}.mp4.temp", self.slug()?));

        let snaptik_token = snaptik_token(client).await?;
        let snaptik_url = snaptik_get_video(client, &snaptik_token, &self.url()).await?;
        let data = client.get(snaptik_url).send().await?.bytes().await?;
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
