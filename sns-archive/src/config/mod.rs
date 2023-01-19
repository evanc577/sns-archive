use std::path::{Path, PathBuf};

use anyhow::Result;
use home_dir::HomeDirExt;
use regex::{Regex, RegexBuilder};
use serde::de::Error;
use serde::{Deserialize, Deserializer};

use self::naver_post::NaverPostConfig;
use self::twitter::TwitterConfig;
use self::weverse::WeverseConfig;
use self::xiaohongshu::XiaoHongShuConfig;
use self::youtube::YoutubeConfig;

pub mod naver_post;
pub mod twitter;
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
    s.expand_home().map_err(D::Error::custom)
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
