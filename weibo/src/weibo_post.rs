use std::borrow::Cow;
use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use indexmap::IndexMap;
use once_cell::sync::Lazy;
use reqwest::{header, Client, Url};
use serde::{Deserialize, Deserializer};
use sns_archive_common::{set_mtime, streamed_download, SavablePost};
use time::format_description::well_known::Rfc3339;
use time::format_description::FormatItem;
use time::{format_description, OffsetDateTime};
use tokio::fs;
use tokio::io::AsyncWriteExt;

#[derive(Deserialize, Debug)]
pub struct WeiboPost {
    #[serde(deserialize_with = "deserialize_datetime")]
    created_at: OffsetDateTime,
    pub id: u64,
    user: WeiboUser,
    #[serde(rename = "text_raw")]
    text: String,
    #[serde(rename = "pic_ids")]
    pictures: Vec<String>,
    #[serde(rename = "url_struct")]
    urls: Option<Vec<WeiboUrl>>,
    #[serde(skip)]
    tid: String,
    #[serde(rename = "isTop")]
    #[serde(deserialize_with = "deserialize_pinned")]
    #[serde(default)]
    pub pinned: bool,
}

impl std::cmp::Ord for WeiboPost {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl std::cmp::PartialOrd for WeiboPost {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::cmp::PartialEq for WeiboPost {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl std::cmp::Eq for WeiboPost {}

impl WeiboPost {
    pub fn set_tid(&mut self, s: String) {
        self.tid = s;
    }
}

fn deserialize_datetime<'de, D>(deserializer: D) -> Result<OffsetDateTime, D::Error>
where
    D: Deserializer<'de>,
{
    static FMT: &str = concat!(
        "[weekday repr:short] [month repr:short] [day] ",
        "[hour repr:24]:[minute]:[second] ",
        "[offset_hour sign:mandatory][offset_minute] [year]",
    );
    static PARSE_FORMAT: Lazy<Vec<FormatItem>> =
        Lazy::new(|| format_description::parse(FMT).unwrap());
    let s = String::deserialize(deserializer)?;
    OffsetDateTime::parse(&s, &PARSE_FORMAT).map_err(serde::de::Error::custom)
}

fn deserialize_pinned<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let v = i64::deserialize(deserializer).map_err(serde::de::Error::custom)?;
    Ok(v == 1)
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct WeiboUser {
    id: u64,
    screen_name: String,
    #[serde(rename = "avatar_hd")]
    avatar: String,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct WeiboUrl {
    #[serde(rename = "url_title")]
    title: String,
    #[serde(rename = "long_url")]
    url: String,
}

#[async_trait]
impl SavablePost for WeiboPost {
    async fn download(&self, client: &Client, directory: impl AsRef<Path> + Send) -> Result<()> {
        // Generate output location
        let slug = self.slug()?;

        // Write text
        {
            let filename = format!("{}-content.txt", &slug);
            let path = directory.as_ref().join(filename);
            self.write_text(path).await?;
        }

        // Download images
        for (i, img_id) in self.pictures.iter().enumerate() {
            let filename = format!("{}-img{:02}.jpg", &slug, i + 1);
            let path = directory.as_ref().join(&filename);
            download_image(client, img_id, path).await?;
        }

        // Download videos
        if let Some(u) = &self.urls {
            for (i, u) in u.iter().enumerate() {
                if let Some(u) = u.is_video() {
                    let filename = format!("{}-vid{:02}", &slug, i + 1);
                    let path = directory.as_ref().join(&filename);
                    download_video(client, &u, path).await?;
                }
            }
        }

        // Set mtime on directory and all files in it
        set_mtime(&directory, &self.created_at)?;
        let mut read_dir = fs::read_dir(directory).await?;
        while let Some(entry) = read_dir.next_entry().await? {
            set_mtime(entry.path(), &self.created_at)?;
        }

        Ok(())
    }

    fn slug(&self) -> Result<String> {
        static FORMAT: Lazy<Vec<FormatItem>> =
            Lazy::new(|| format_description::parse("[year][month][day]").unwrap());
        let date = self.created_at.format(&FORMAT)?;
        let prefix = format!("{}-{}-{}", date, self.id, self.user.screen_name);
        Ok(prefix)
    }
}

impl WeiboPost {
    async fn write_text(&self, path: impl AsRef<Path>) -> Result<()> {
        let url = format!("https://m.weibo.cn/status/{}", self.id);
        let time = self.created_at.format(&Rfc3339)?;

        let mut file = fs::File::create(path).await?;
        file.write_all(format!("url: {}\n", url).as_bytes()).await?;
        file.write_all(format!("user: {}\n", self.user.screen_name).as_bytes())
            .await?;
        file.write_all(format!("created_at: {}\n", time).as_bytes())
            .await?;
        if let Some(u) = &self.urls {
            for u in u {
                if !u.url.is_empty() {
                    file.write_all(format!("link: {}\n", u.url).as_bytes())
                        .await?;
                }
            }
        }
        file.write_all(format!("\n{}", self.text).as_bytes())
            .await?;

        Ok(())
    }
}

impl WeiboUrl {
    fn is_video(&'_ self) -> Option<Cow<'_, str>> {
        if self.url.starts_with("https://video.weibo.com") {
            let fid = Url::parse(&self.url)
                .unwrap()
                .query_pairs()
                .find(|f| f.0 == "fid")
                .unwrap()
                .1
                .to_string();
            return Some(Cow::from(format!(
                "https://weibo.com/tv/show/{}?from=old_pc_videoshow",
                fid
            )));
        } else if self.url.starts_with("https://weibo.com/tv/show/") {
            return Some(Cow::from(&self.url));
        }

        None
    }
}

async fn download_image(client: &Client, img_id: &str, path: impl AsRef<Path>) -> Result<()> {
    // Download
    let url = format!("https://wx2.sinaimg.cn/large/{img_id}.jpg");
    let data = client.get(url).send().await?.bytes().await?;

    // Write
    let mut file = fs::File::create(path.as_ref()).await?;
    file.write_all(&data).await.map_err(|_| {
        anyhow::anyhow!(format!(
            "Could not write to {}",
            path.as_ref().to_string_lossy()
        ))
    })?;

    Ok(())
}

async fn download_video(client: &Client, url: &str, path: impl AsRef<Path>) -> Result<()> {
    #[derive(Deserialize)]
    struct WeiboVideo {
        data: WeiboData,
    }

    #[derive(Deserialize)]
    struct WeiboData {
        #[serde(rename = "Component_Play_Playinfo")]
        play_info: WeiboPlayInfo,
    }

    #[derive(Deserialize)]
    struct WeiboPlayInfo {
        urls: IndexMap<String, String>,
    }

    let parsed_url = Url::parse(url)?;
    let data_url = format!(
        "https://weibo.com/tv/api/component?page={}",
        parsed_url.path()
    );
    let id = parsed_url.path_segments().unwrap().next_back().unwrap();
    let cookie = "SUB=_";
    let content_type = "application/x-www-form-urlencoded";
    let data = format!(r#"data={{"Component_Play_Playinfo":{{"oid":"{}"}}}}"#, id);

    let mut video_url = client
        .post(data_url)
        .header(header::REFERER, url)
        .header(header::COOKIE, cookie)
        .header(header::CONTENT_TYPE, content_type)
        .body(data)
        .send()
        .await?
        .json::<WeiboVideo>()
        .await?
        .data
        .play_info
        .urls
        .first()
        .unwrap()
        .1
        .clone();

    if !video_url.starts_with("http") {
        video_url = format!("https:{}", video_url)
    }

    streamed_download(client, video_url, path).await?;

    Ok(())
}
