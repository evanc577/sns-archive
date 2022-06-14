use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Deserializer};

use self::twitter::TwitterConfig;
use self::weverse::WeverseConfig;
use self::youtube::YoutubeConfig;

pub mod twitter;
pub mod weverse;
pub mod youtube;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub weverse: Option<WeverseConfig>,
    pub twitter: Option<TwitterConfig>,
    pub youtube: Option<YoutubeConfig>,
}

impl Config {
    pub fn read(path: impl AsRef<Path>) -> Result<Self> {
        let conf_contents = std::fs::read_to_string(path.as_ref())?;
        Ok(toml::from_str(&conf_contents)?)
    }
}

fn deserialize_directory<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    Ok(PathBuf::from(shellexpand::tilde(&s).as_ref()))
}
