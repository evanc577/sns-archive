use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;

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
    #[serde(deserialize_with = "super::deserialize_path")]
    pub artist_download_path: PathBuf,
    #[serde(deserialize_with = "super::deserialize_path")]
    pub moments_download_path: PathBuf,
}

fn default_num_processes() -> usize {
    20
}
