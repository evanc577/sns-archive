use std::path::Path;

use anyhow::Result;
use serde::Deserialize;

use self::twitter::TwitterConfig;
use self::weverse::WeverseConfig;
use self::youtube::YoutubeConfig;

pub mod weverse;
pub mod twitter;
pub mod youtube;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub weverse: WeverseConfig,
    pub twitter: TwitterConfig,
    pub youtube: YoutubeConfig,
}

impl Config {
    pub fn read(path: impl AsRef<Path>) -> Result<Self> {
        let conf_contents = std::fs::read_to_string(path.as_ref())?;
        Ok(toml::from_str(&conf_contents)?)
    }
}
