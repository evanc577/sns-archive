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
    #[serde(deserialize_with = "super::deserialize_directory")]
    pub download_path: PathBuf,
}
