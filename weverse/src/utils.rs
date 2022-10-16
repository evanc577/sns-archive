use anyhow::Result;
use serde::{Deserialize, Deserializer};
use time::{format_description, OffsetDateTime, UtcOffset};
use unicode_segmentation::UnicodeSegmentation;

use crate::endpoint::member::Member;

pub(crate) fn deserialize_timestamp<'de, D>(deserializer: D) -> Result<OffsetDateTime, D::Error>
where
    D: Deserializer<'de>,
{
    let ts = i128::deserialize(deserializer)? * 1_000_000;
    let dt = OffsetDateTime::from_unix_timestamp_nanos(ts)
        .map_err(serde::de::Error::custom)?
        // KST
        .to_offset(UtcOffset::from_hms(9, 0, 0).map_err(serde::de::Error::custom)?);
    Ok(dt)
}

pub(crate) fn slug(
    time: &OffsetDateTime,
    post_id: &str,
    author: &Member,
    body: &str,
) -> Result<String> {
    let time_str = {
        let format = format_description::parse("[year][month][day]")?;
        time.format(&format)?
    };
    let username = &author.official_profile.official_name;
    let body: String = UnicodeSegmentation::graphemes(body, true)
        .take(50)
        .collect();
    let slug = format!("{}-{}-{}-{}", time_str, post_id, username, body);
    let sanitize_options = sanitize_filename::Options {
        windows: true,
        ..Default::default()
    };
    let sanitized_slug = sanitize_filename::sanitize_with_options(slug, sanitize_options);
    Ok(sanitized_slug)
}

#[cfg(test)]
use async_once_cell::OnceCell;

#[cfg(test)]
pub static LOGIN_INFO: OnceCell<String> = OnceCell::new();

#[cfg(test)]
pub async fn setup() -> String {
    use dotenv::dotenv;
    use reqwest::Client;

    use crate::auth::{login, LoginInfo};

    let _ = dotenv();
    let email = std::env::var("WEVERSE_EMAIL").unwrap();
    let password = std::env::var("WEVERSE_PASSWORD").unwrap();
    let login_info = LoginInfo { email, password };

    let client = Client::new();
    let auth = login(&client, &login_info).await.unwrap();
    format!("Bearer {}", auth)
}
