use anyhow::Result;
use serde::{Deserialize, Deserializer};

#[derive(Deserialize, Debug)]
pub struct TwitterConfig {
    pub bearer: String,
    #[serde(deserialize_with = "deserialize_directory")]
    pub download_path: String,
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
