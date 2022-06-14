use std::path::Path;

use anyhow::Result;
use serde::Deserialize;

use self::weverse::WeverseConfig;

pub mod weverse;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub weverse: WeverseConfig,
}

impl Config {
    pub fn read(path: impl AsRef<Path>) -> Result<Self> {
        let conf_contents = std::fs::read_to_string(path.as_ref())?;
        Ok(toml::from_str(&conf_contents)?)
    }
}
