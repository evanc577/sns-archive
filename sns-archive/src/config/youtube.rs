use std::path::PathBuf;

use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct YoutubeConfig {
    #[serde(deserialize_with = "super::deserialize_directory")]
    pub download_path: PathBuf,
    pub channels: Vec<YTChannel>,
    pub filter: String,
}

#[derive(Deserialize, Debug)]
pub struct YTChannel {
    pub channel_id: String,
    pub display_name: String,
    #[serde(default)]
    pub apply_filter: bool,
    #[serde(default)]
    pub always_redownload: bool,
    pub custom_filter: Option<String>,
    pub playlist_end: Option<usize>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

const fn default_true() -> bool {
    true
}
