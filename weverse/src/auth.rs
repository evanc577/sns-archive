use anyhow::Result;
use hmac::{Hmac, Mac};
use lazy_static::lazy_static;
use regex::Regex;
use reqwest::{Client, Url};
use sha1::Sha1;
use time::OffsetDateTime;

use crate::error::WeverseError;

lazy_static! {
    static ref JS_RE: Regex = Regex::new(r#"src="(?P<url>.+/main.*\.js)""#).unwrap();
    static ref SECRET_RE: Regex = Regex::new(r#"return\s?"(?P<key>[a-fA-F0-9]{16,})""#).unwrap();
}

pub async fn get_secret(client: &Client) -> Result<Vec<u8>> {
    // Get js script
    let resp = client
        .get("https://weverse.io")
        .send()
        .await?
        .text()
        .await?;
    let js_url = JS_RE
        .captures(&resp)
        .ok_or(WeverseError::Auth)?
        .name("url")
        .ok_or(WeverseError::Auth)?
        .as_str();

    // Extract key from js script
    let resp = client.get(js_url).send().await?.text().await?;
    let key = SECRET_RE
        .captures(&resp)
        .ok_or(WeverseError::Auth)?
        .name("key")
        .ok_or(WeverseError::Auth)?
        .as_str()
        .as_bytes()
        .to_vec();

    Ok(key)
}

pub async fn compute_url(base_url: &str, secret: &[u8]) -> Result<Url> {
    let pad = OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
    let url_hash = format!("{}{}", base_url, &pad.to_string());

    let mut mac = Hmac::<Sha1>::new_from_slice(secret)?;
    mac.update(url_hash.as_bytes());
    let digest = base64::encode(mac.finalize().into_bytes());

    static DOMAIN: &str = "https://global.apis.naver.com/weverse/wevweb";
    let url = format!("{}{}", DOMAIN, base_url);

    let mut url = Url::parse(&url)?;
    url.query_pairs_mut()
        .append_pair("wmsgpad", &pad.to_string())
        .append_pair("wmd", &digest);

    Ok(url)
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn check() {
        let client = Client::new();
        assert!(get_secret(&client).await.is_ok())
    }
}
