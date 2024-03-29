use std::path::{Path, PathBuf};

use anyhow::Result;
#[cfg(target_family = "unix")]
use home_dir::HomeDirExt;
use regex::{Regex, RegexBuilder};
use serde::de::Error;
use serde::{Deserialize, Deserializer};

use self::naver_post::NaverPostConfig;
use self::tiktok::TikTokConfig;
use self::twitter::TwitterConfig;
use self::weibo::WeiboConfig;
use self::weverse::WeverseConfig;
use self::xiaohongshu::XiaoHongShuConfig;
use self::youtube::YoutubeConfig;

pub mod naver_post;
pub mod tiktok;
pub mod twitter;
pub mod weibo;
pub mod weverse;
pub mod xiaohongshu;
pub mod youtube;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub weverse: Option<WeverseConfig>,
    pub twitter: Option<TwitterConfig>,
    pub youtube: Option<YoutubeConfig>,
    pub naver_post: Option<NaverPostConfig>,
    pub xiaohongshu: Option<XiaoHongShuConfig>,
    pub weibo: Option<WeiboConfig>,
    pub tiktok: Option<TikTokConfig>,
}

impl Config {
    pub fn read(path: impl AsRef<Path>) -> Result<Self> {
        let conf_contents = std::fs::read_to_string(path.as_ref())?;
        Ok(toml::from_str(&conf_contents)?)
    }
}

fn deserialize_path<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
where
    D: Deserializer<'de>,
{
    let s: PathBuf = Deserialize::deserialize(deserializer)?;
    expand_home::<D>(s)
}

fn deserialize_option_path<'de, D>(deserializer: D) -> Result<Option<PathBuf>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<PathBuf> = Deserialize::deserialize(deserializer)?;
    s.map(|s| expand_home::<D>(s)).transpose()
}

#[cfg(target_family = "unix")]
fn expand_home<'de, D>(path: impl AsRef<Path>) -> Result<PathBuf, D::Error>
where
    D: Deserializer<'de>,
{
    path.expand_home().map_err(D::Error::custom)
}

#[cfg(target_family = "windows")]
fn expand_home<'de, D>(path: impl AsRef<Path>) -> Result<PathBuf, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(path.as_ref().into())
}


fn deserialize_regex_option<'de, D>(deserializer: D) -> Result<Option<Regex>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    let r = RegexBuilder::new(&s)
        .case_insensitive(true)
        .build()
        .map_err(D::Error::custom)?;
    Ok(Some(r))
}
