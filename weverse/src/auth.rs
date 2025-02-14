use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use base64::engine::general_purpose;
use base64::Engine as _;
use hmac::{Hmac, Mac};
use lazy_static::lazy_static;
use regex::Regex;
use reqwest::{header, Client, Url};
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use time::OffsetDateTime;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use crate::endpoint::me::me;
use crate::error::WeverseError;

lazy_static! {
    static ref JS_RE: Regex = Regex::new(r#"src="(?P<url>[^"]*js/main.\w+\.js[^"]*)""#).unwrap();
    // TODO: Think of a better way to extract hmac secret
    static ref SECRET_RE: Regex = Regex::new(r#"(?P<key>[a-f0-9]{40})"#).unwrap();
    // static ref SECRET_RE: Regex = Regex::new(r#"return\s?"(?P<key>[a-fA-F0-9]{16,})""#).unwrap();
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
        .and_then(|x| x.name("key"))
        .map(|x| x.as_str().as_bytes().to_vec())
        .ok_or(WeverseError::Auth)?;

    Ok(key)
}

pub(crate) async fn compute_url(base_url: &str, secret: &[u8]) -> Result<Url> {
    let pad = OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
    let data_to_hash: Vec<_> = base_url
        .as_bytes()
        .iter()
        .take(255)
        .chain(pad.to_string().as_bytes().iter())
        .copied()
        .collect();

    let mut mac = Hmac::<Sha1>::new_from_slice(secret)?;
    mac.update(&data_to_hash);
    let digest = general_purpose::STANDARD.encode(mac.finalize().into_bytes());

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
        Regex::new(r#"\bsrc="(?P<path>[^"]*_app-[0-9a-zA-Z]+\.js)""#).unwrap();
    static ref APP_SECRET_RE: Regex =
        Regex::new(r#"\bAPP_SECRET,\s*"(?P<secret>[0-9a-zA-Z]+)""#).unwrap();
    static ref APP_VERSION_RE: Regex =
        Regex::new(r#"\bAPP_VERSION,\s*"(?P<version>[0-9\.]+)""#).unwrap();
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LoginInfo {
    email: String,
}

impl LoginInfo {
    pub fn new(email: &str) -> Self {
        LoginInfo {
            email: email.to_owned(),
        }
    }
}

pub(crate) async fn login(client: &Client, login_info: &LoginInfo) -> Result<String> {
    // Check saved authorization
    let auth = if let Ok(Some(auth)) = load_saved_authorization(&login_info.email).await {
        // Check login status
        validate_or_refresh_bearer( client, &login_info.email, &auth).await?
    } else {
        return Err(WeverseError::Login)?;
    };

    // Disable password login because it requires a captcha now
    // let access_token = email_login(client, login_info).await?;

    // Check login status
    me(client, &auth.authorization).await?;

    // Save authorization
    store_authorization(&login_info.email, &auth.authorization, &auth.refresh).await?;

    Ok(auth.authorization)
}

async fn validate_or_refresh_bearer(
    client: &Client,
    username: &str,
    auth: &SavedAuthorization,
) -> Result<SavedAuthorization> {
    #[derive(Deserialize, Clone, Debug)]
    #[serde(rename_all = "camelCase")]
    struct ValidateResponse {
        refresh_required: bool,
    }
    let valid: ValidateResponse = client
        .get("https://accountapi.weverse.io/api/v1/token/validate")
        .header("x-acc-service-id", "weverse")
        .header(header::AUTHORIZATION, &format!("Bearer {}", auth.authorization))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    if !valid.refresh_required {
        // Return current auth if not update is required
        return Ok(auth.clone());
    }

    // Get a new bearer token with using the refresh token
    #[derive(Serialize, Clone, Debug)]
    #[serde(rename_all = "camelCase")]
    struct RefreshRequestData {
        refresh_token:  String,
    }
    #[derive(Deserialize, Clone, Debug)]
    #[serde(rename_all = "camelCase")]
    struct RefreshResponse {
        access_token: String,
        refresh_token:  String,
    }
    let new_token: RefreshResponse = client
        .post("https://accountapi.weverse.io/api/v1/token/refresh")
        .json(&RefreshRequestData { refresh_token: auth.refresh.clone()})
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let new_saved_auth = SavedAuthorization {
        authorization: new_token.access_token,
        refresh: new_token.refresh_token,
    };
    store_authorization(username, &auth.authorization, &auth.refresh).await?;

    Ok(new_saved_auth)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct SavedAuthorization {
    authorization: String,
    refresh: String,
}

static SAVED_AUTH_FILE_NAME: &str = "sns-archive/weverse_logins";

fn saved_authorization_file() -> Result<PathBuf> {
    directories::BaseDirs::new()
        .map(|d| d.data_dir().join(SAVED_AUTH_FILE_NAME))
        .ok_or_else(|| WeverseError::SavedAuthFile.into())
}

async fn load_saved_authorization(username: &str) -> Result<Option<SavedAuthorization>> {
    let filename = saved_authorization_file()?;
    let contents = fs::read_to_string(filename).await?;
    let authorizations: HashMap<String, SavedAuthorization> = toml::from_str(&contents)?;
    Ok(authorizations.get(username).cloned())
}

lazy_static! {
    static ref AUTH_FILE_MTX: Mutex<()> = Mutex::new(());
}

async fn store_authorization(username: &str, authoriazation: &str, refresh: &str) -> Result<()> {
    let _guard = AUTH_FILE_MTX.lock().await;

    let filename = saved_authorization_file()?;
    fs::create_dir_all(&filename.parent().unwrap()).await?;
    let contents = fs::read_to_string(&filename).await.unwrap_or_default();
    let mut authorizations: HashMap<String, SavedAuthorization> = toml::from_str(&contents)?;
    authorizations.insert(
        username.to_string(),
        SavedAuthorization {
            authorization: authoriazation.to_string(),
            refresh: refresh.to_string(),
        },
    );

    let mut file = fs::File::create(filename).await?;
    file.write_all(toml::to_string(&authorizations)?.as_bytes())
        .await?;

    Ok(())
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
        // let password = std::env::var("WEVERSE_PASSWORD").unwrap();
        let login_info = LoginInfo { email };

        let client = Client::new();
        login(&client, &login_info).await.unwrap();
    }
}
