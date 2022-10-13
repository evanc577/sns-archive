use serde::{Deserialize, Deserializer};
use time::OffsetDateTime;

use crate::endpoint::community_id::CommunityId;

pub(crate) fn deserialize_timestamp<'de, D>(deserializer: D) -> Result<OffsetDateTime, D::Error>
where
    D: Deserializer<'de>,
{
    let ts = i128::deserialize(deserializer)? * 1_000_000;
    OffsetDateTime::from_unix_timestamp_nanos(ts).map_err(serde::de::Error::custom)
}

pub(crate) fn deserialize_community_id<'de, D>(deserializer: D) -> Result<CommunityId, D::Error>
where
    D: Deserializer<'de>,
{
    let id = u64::deserialize(deserializer)?;
    Ok(CommunityId::new(id))
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
