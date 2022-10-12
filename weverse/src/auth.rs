use anyhow::Result;
use hmac::{Hmac, Mac};
use lazy_static::lazy_static;
use regex::Regex;
use reqwest::{header, Client, Url};
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::WeverseError;

lazy_static! {
    static ref JS_RE: Regex = Regex::new(r#"src="(?P<url>.+/main.*\.js)""#).unwrap();
    static ref SECRET_RE: Regex = Regex::new(r#"return\s?"(?P<key>[a-fA-F0-9]{16,})""#).unwrap();
}

pub(crate) async fn get_secret(client: &Client) -> Result<Vec<u8>> {
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
        .unwrap()
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

pub(crate) async fn compute_url(base_url: &str, secret: &[u8]) -> Result<Url> {
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

lazy_static! {
    static ref APP_JS_RE: Regex =
        Regex::new(r#"\bsrc="(?P<path>.+?/_app-[0-9a-zA-Z]+\.js)""#).unwrap();
    static ref APP_SECRET_RE: Regex =
        Regex::new(r#"\bAPP_SECRET,\s*"(?P<secret>[0-9a-zA-Z]+)""#).unwrap();
    static ref APP_VERSION_RE: Regex =
        Regex::new(r#"\bAPP_VERSION,\s*"(?P<version>[0-9\.]+)""#).unwrap();
}

#[derive(Serialize, Clone, Debug)]
pub struct LoginInfo {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LoginResponse {
    access_token: String,
}

pub(crate) async fn login(client: &Client, login_info: &LoginInfo) -> Result<String> {
    // Extract app secrets
    let login_page = client
        .get("https://account.weverse.io/en/signup")
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let js_path = APP_JS_RE
        .captures(&login_page)
        .ok_or(WeverseError::Login)?
        .name("path")
        .unwrap()
        .as_str();
    let url = format!("https://account.weverse.io{}", js_path);
    let app_js = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let x_acc_app_secret = APP_SECRET_RE
        .captures(&app_js)
        .ok_or(WeverseError::Login)?
        .name("secret")
        .unwrap()
        .as_str();
    let x_acc_app_version = APP_VERSION_RE
        .captures(&app_js)
        .ok_or(WeverseError::Login)?
        .name("version")
        .unwrap()
        .as_str();

    // Generate uuids
    let x_acc_trace_id = Uuid::new_v4().to_string();
    let x_clog_user_device_id = Uuid::new_v4().to_string();

    // Login
    let access_token = client
        .post("https://accountapi.weverse.io/web/api/v2/auth/token/by-credentials")
        .header(header::REFERER, "https://account.weverse.io/")
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-acc-app-secret", x_acc_app_secret)
        .header("x-acc-app-version", x_acc_app_version)
        .header("x-acc-language", "en")
        .header("x-acc-service-id", "weverse")
        .header("x-acc-trace-id", x_acc_trace_id)
        .header("x-clog-user-device-id", x_clog_user_device_id)
        .json(login_info)
        .send()
        .await?
        .error_for_status()?
        .json::<LoginResponse>()
        .await?
        .access_token;

    Ok(access_token)
}

#[cfg(test)]
mod test {
    use dotenv::dotenv;

    use super::*;

    #[tokio::test]
    async fn endpoint_auth() {
        let client = Client::new();
        assert!(get_secret(&client).await.is_ok())
    }

    #[tokio::test]
    async fn try_login() {
        // Read secrets
        let _ = dotenv();
        let email = std::env::var("WEVERSE_EMAIL").unwrap();
        let password = std::env::var("WEVERSE_PASSWORD").unwrap();
        let login_info = LoginInfo { email, password };

        let client = Client::new();
        assert!(login(&client, &login_info).await.is_ok());
    }
}
