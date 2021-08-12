use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub bearer: String,
    pub directory: String,
    pub users: Vec<String>,
    #[serde(default)]
    pub timezone_offset: i32,
}

impl Config {
    pub fn read() -> Result<Config> {
        let path = "config.toml";
        let data = std::fs::read_to_string(path).context(format!("Failed to read {}", path))?;
        Ok(toml::from_str(&data).context(format!("Failed to parse {}", path))?)
    }
}
