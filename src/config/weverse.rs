use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use lazy_static::lazy_static;
use regex::Regex;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct WeverseConfig {
    pub cookies_file: String,
    #[serde(default = "default_keep_open")]
    pub keep_open: bool,
    #[serde(default = "default_num_processes")]
    pub max_connections: usize,
    pub artists: HashMap<String, ArtistConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ArtistConfig {
    #[serde(deserialize_with = "super::deserialize_directory")]
    pub artist_download_path: PathBuf,
    #[serde(deserialize_with = "super::deserialize_directory")]
    pub moments_download_path: PathBuf,
    pub recent_artist: Option<isize>,
    pub recent_moments: Option<isize>,
}

fn default_keep_open() -> bool {
    false
}
fn default_num_processes() -> usize {
    20
}

pub fn read_token(cookies_file: impl AsRef<Path>) -> Result<String, String> {
    lazy_static! {
        static ref RE: Regex = Regex::new(
            r"(?m)^(?P<domain>\.weverse\.io)\t.+?\t.+?\t.+?\t.+?\t(?P<name>we_access_token)\t(?P<value>.+?)$"
        ).unwrap();
    }

    let cookies_contents = fs::read_to_string(&cookies_file)
        .map_err(|e| format!("Error reading {:?}: {}", cookies_file.as_ref(), e))?;

    let token = RE
        .captures(&cookies_contents)
        .ok_or(format!("Error parsing {:?}", cookies_file.as_ref()))?
        .name("value")
        .ok_or(format!("Error applying regex for {:?}", cookies_file.as_ref()))?
        .as_str()
        .to_owned();

    Ok(token)
}
