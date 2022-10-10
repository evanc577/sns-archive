use std::path::PathBuf;

use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct TwitterConfig {
    pub bearer: String,
    #[serde(deserialize_with = "super::deserialize_directory")]
    pub download_path: PathBuf,
    pub users: Vec<String>,
    #[serde(default)]
    pub timezone_offset: i32,
}
