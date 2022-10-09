use serde::{Deserialize, Deserializer};
use std::cmp::Ordering;

#[derive(Deserialize, Debug)]
pub struct Tweet {
    pub created_at: String,
    pub id: u64,
    pub id_str: String,
    pub full_text: String,
    pub truncated: bool,
    pub display_text_range: Vec<u64>,
    pub entities: Entities,
    pub extended_entities: Option<ExtendedEntities>,
    pub user: User,
    #[serde(deserialize_with = "deserialize_retweet_status")]
    #[serde(default)]
    pub retweeted_status: bool,
}

#[derive(Deserialize, Debug)]
pub struct Entities {
    pub hashtags: Vec<HashtagEntity>,
    pub urls: Vec<UrlEntity>,
}

#[derive(Deserialize, Debug)]
pub struct HashtagEntity {
    pub indices: Vec<u64>,
    pub text: String,
}

#[derive(Deserialize, Debug)]
pub struct UrlEntity {
    pub display_url: String,
    pub expanded_url: String,
    pub indices: Vec<u64>,
    pub url: String,
}

#[derive(Deserialize, Debug)]
pub struct ExtendedEntities {
    pub media: Vec<Media>,
}

#[derive(Deserialize, Debug)]
pub struct Media {
    pub expanded_url: String,
    pub media_url: String,
    pub media_url_https: String,
    #[serde(rename = "type")]
    pub media_type: String,
    pub url: String,
    pub video_info: Option<VideoInfo>,
}

#[derive(Deserialize, Debug)]
pub struct VideoInfo {
    pub aspect_ratio: Vec<u64>,
    pub duration_millis: Option<u64>,
    pub variants: Vec<VideoVariants>,
}

#[derive(Deserialize, Debug)]
pub struct VideoVariants {
    pub bitrate: Option<u64>,
    pub content_type: String,
    pub url: String,
}

#[derive(Deserialize, Debug)]
pub struct User {
    pub created_at: String,
    pub description: String,
    pub favourites_count: u64,
    pub followers_count: u64,
    pub friends_count: u64,
    pub has_extended_profile: bool,
    pub id: u64,
    pub id_str: String,
    pub listed_count: u64,
    pub name: String,
    pub screen_name: String,
    pub verified: bool,
}

fn deserialize_retweet_status<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let s: serde_json::Value = Deserialize::deserialize(deserializer)?;
    Ok(!s.is_null())
}

impl PartialEq for Tweet {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl PartialOrd for Tweet {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.id.cmp(&other.id))
    }
}

impl Eq for Tweet {}

impl Ord for Tweet {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}
