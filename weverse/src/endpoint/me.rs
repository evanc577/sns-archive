use anyhow::Result;
use reqwest::{header, Client};
use serde::Deserialize;

use crate::{
    auth::{compute_url, get_secret},
    client,
};

use super::{APP_ID, REFERER};

#[allow(dead_code)]
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Me {
    wid: String,
    user_key: String,
    country: String,
    first_name: String,
    last_name: String,
    nickname: String,
    status: String,
}

pub(crate) async fn me(client: &Client, auth: &str) -> Result<Me> {
    let secret = get_secret(client).await?;
    let url = compute_url(
        &format!(
            "/users/v1.0/users/me?appId={}&language=en&platform=WEB&wpf=pc",
            APP_ID
        ),
        &secret,
    )
    .await?;

    let me = client
        .get(url.as_str())
        .header(header::REFERER, REFERER)
        .header(header::AUTHORIZATION, auth)
        .send()
        .await?
        .error_for_status()?
        .json::<Me>()
        .await?;

    Ok(me)
}
