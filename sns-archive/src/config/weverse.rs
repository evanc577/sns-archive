use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;

/// Download Weverse posts and moments
#[derive(Debug, Deserialize, Clone)]
pub struct WeverseConfig {
    pub email: String,
    pub password: String,
    #[serde(default = "default_num_processes")]
    pub max_connections: usize,
    pub artists: HashMap<String, ArtistConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ArtistConfig {
    #[serde(deserialize_with = "super::deserialize_option_path")]
    #[serde(default)]
    pub artist_download_path: Option<PathBuf>,
    #[serde(deserialize_with = "super::deserialize_option_path")]
    #[serde(default)]
    pub moments_download_path: Option<PathBuf>,
}

fn default_num_processes() -> usize {
    20
}
