use anyhow::Result;
use reqwest::Client;

use crate::auth::{compute_url, get_secret};

pub enum WeverseEndpoints {
    Vod(String),
}
