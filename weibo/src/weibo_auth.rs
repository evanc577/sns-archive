use anyhow::Result;
use regex::Regex;
use reqwest::Client;
use serde::Deserialize;

pub struct WeiboAuth {
    pub tid: String,
    pub cookies: String,
}

pub async fn weibo_cookie(client: &Client) -> Result<WeiboAuth> {
    static URL: &str = "https://passport.weibo.com/visitor/genvisitor2";
    let re = Regex::new(r"(?:visitor_gray_callback|gen_callback)\((.*)\)").unwrap();

    #[derive(Deserialize)]
    struct ResponseJson {
        data: Data,
    }

    #[derive(Deserialize)]
    struct Data {
        sub: String,
        tid: String,
    }

    // POST to Weibo
    let text = client
        .post(URL)
        .form(&[("cb", "gen_callback")])
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    // Extract and parse JSON
    let json: ResponseJson = serde_json::from_str(
        re.captures(&text)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse genvisitor output"))?
            .get(1)
            .unwrap()
            .as_str(),
    )?;

    let auth = WeiboAuth {
        tid: json.data.tid,
        cookies: json.data.sub,
    };
    Ok(auth)
}
