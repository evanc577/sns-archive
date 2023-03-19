use std::path::PathBuf;

use serde::Deserialize;

/// Download Weverse posts and moments
#[derive(Debug, Deserialize, Clone)]
pub struct WeiboConfig {
    #[serde(default = "default_num_processes")]
    pub max_connections: usize,
    pub users: Vec<WeiboUserConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WeiboUserConfig {
    pub user: u64,
    #[serde(deserialize_with = "super::deserialize_path")]
    pub download_path: PathBuf,
}

fn default_num_processes() -> usize {
    20
}
