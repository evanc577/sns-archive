use std::fs::File;
use std::io;
use std::io::{BufRead, Write};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Deserializer};
use thirtyfour::cookie::Cookie;
use thirtyfour::prelude::*;
use thirtyfour::{CapabilitiesHelper, DesiredCapabilities, FirefoxCapabilities, WebDriver};
use tokio::io::AsyncWriteExt;
use tokio::{fs, process, time};
use unicode_segmentation::UnicodeSegmentation;

use crate::config::weverse::WeverseConfig;

static DRIVER_ADDR: &str = "http://localhost:4444";

pub async fn download(config: &WeverseConfig) -> Result<()> {
    let _driver = process::Command::new("geckodriver")
        .arg("--port=4444")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let mut caps = DesiredCapabilities::firefox();
    caps.set("pageLoadStrategy".into(), serde_json::json!("none"));
    let driver = loop {
        match WebDriver::new(DRIVER_ADDR, caps.clone()).await {
            Ok(d) => break d,
            _ => time::sleep(Duration::from_secs(1)).await,
        }
        println!("Waiting for driver...");
    };

    login(&driver, &config.cookies_file).await?;
    open_page(&driver).await?;

    // Close window
    driver.close().await?;
    driver.quit().await?;

    Ok(())
}

async fn login(driver: &WebDriver, cookies_file: impl AsRef<Path>) -> Result<Vec<Cookie<'_>>> {
    driver.get("about:blank").await?;
    driver.get("https://weverse.io").await?;

    let cookies = if cookies_file.as_ref().exists() {
        let file = File::open(cookies_file)?;
        let cookies: Vec<_> = io::BufReader::new(file)
            .lines()
            .filter_map(|l| {
                let line = l.ok()?;
                Cookie::parse(line).ok()
            })
            .collect();

        for cookie in cookies.iter() {
            driver.add_cookie(cookie.clone()).await?;
        }

        cookies
    } else {
        // Let user log in
        let cookies = loop {
            let cookies = driver.get_cookies().await?;
            if cookies.iter().any(|c| c.name() == "we2_access_token") {
                break cookies;
            }

            tokio::time::sleep(Duration::from_secs(1)).await;
        };

        // Save cookies to file
        let mut file = File::create(cookies_file)?;
        for cookie in cookies.iter() {
            writeln!(&mut file, "{}", cookie)?;
        }

        cookies
    };

    Ok(cookies)
}

async fn open_page(driver: &WebDriver) -> Result<()> {
    driver.get("https://weverse.io/dreamcatcher/artist").await?;
    tokio::time::sleep(Duration::from_secs(1000000)).await;
    Ok(())
}
