use std::path::PathBuf;

use regex::Regex;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct NaverPostConfig {
    pub members: Vec<NPMember>,
}

#[derive(Deserialize, Debug)]
pub struct NPMember {
    pub id: String,
    #[serde(default, deserialize_with = "super::deserialize_regex_option")]
    pub filter: Option<Regex>,
    pub limit: Option<usize>,
    #[serde(deserialize_with = "super::deserialize_path")]
    pub download_path: PathBuf,
    pub until_post: Option<u64>,
    pub since_post: Option<u64>,
}
