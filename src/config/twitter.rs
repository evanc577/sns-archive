use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer};

#[derive(Deserialize, Debug)]
pub struct TwitterConfig {
    pub bearer: String,
    #[serde(deserialize_with = "deserialize_directory")]
    pub directory: String,
    pub users: Vec<String>,
    #[serde(default)]
    pub timezone_offset: i32,
}

fn deserialize_directory<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    Ok(shellexpand::tilde(&s).to_string())
}

impl TwitterConfig {
    pub fn read() -> Result<TwitterConfig> {
        let path = "config.toml";
        let data = std::fs::read_to_string(path).context(format!("Failed to read {}", path))?;
        Ok(toml::from_str(&data).context(format!("Failed to parse {}", path))?)
    }
}
