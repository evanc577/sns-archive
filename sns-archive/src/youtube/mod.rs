use std::fmt::Display;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::config::youtube::{YTChannel, YoutubeConfig};

#[derive(Debug)]
pub struct YTError(Vec<YTChannel>);

impl Display for YTError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "failed to download {} channels:", self.0.len())?;
        for channel in &self.0 {
            eprintln!("  {} ({})", channel.channel_id, channel.display_name);
        }
        Ok(())
    }
}

impl std::error::Error for YTError {}

pub fn download(config: YoutubeConfig) -> Result<(), YTError> {
    let channels = config.channels;
    let filter = config.filter;
    let mut errored_channels = vec![];

    for channel in channels {
        if !channel.enabled {
            continue;
        }

        // Check if new channel
        let new_channel = !directory_exists(&channel.display_name);
        let tmp_dir = format!(".{}.temp", &channel.display_name);
        let target_dir = match new_channel {
            true => {
                fs::create_dir_all(&tmp_dir).unwrap();
                &tmp_dir
            }
            false => &channel.display_name,
        };

        // Build and run yt-dl command
        let args = generate_cmd_args(&channel, &target_dir, &filter, new_channel);
        let output = Command::new("yt-dlp")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .args(&args)
            .output()
            .expect("Failed to execute command");
        if !output.status.success() {
            eprintln!("{}", output.status);
            errored_channels.push(channel);
            continue;
        }

        if new_channel {
            fs::rename(&tmp_dir, &channel.display_name).expect(&format!(
                "Could not rename directory {} to {}",
                &tmp_dir, &channel.display_name
            ));
        }
    }

    if errored_channels.is_empty() {
        Ok(())
    } else {
        Err(YTError(errored_channels))
    }
}

fn generate_cmd_args(
    channel: &YTChannel,
    target_dir: &str,
    default_filter: &str,
    new_channel: bool,
) -> Vec<String> {
    let mut args = vec![
        "--format-sort".to_string(),
        "res,fps,vcodec,acodec".to_string(),
        "--ignore-config".to_string(),
        "--all-subs".to_string(),
        "--embed-subs".to_string(),
        "--compat-options".to_string(),
        "no-live-chat".to_string(),
        "--ignore-errors".to_string(),
        "--match-filter".to_string(),
        "!is_live".to_string(),
        "--remux-video".to_string(),
        "mkv".to_string(),
        "--output".to_string(),
        Path::new(&target_dir)
            .join("%(upload_date)s_%(title)s_%(id)s.%(ext)s")
            .to_str()
            .unwrap()
            .to_owned(),
    ];

    if channel.apply_filter {
        args.push("--match-title".to_string());
        match &channel.custom_filter {
            Some(f) => args.push(f.to_string()),
            None => args.push(default_filter.to_string()),
        }
    }

    if !channel.always_redownload {
        args.push("--download-archive".to_string());
        args.push("downloaded.txt".to_string());
    }

    if !new_channel {
        args.push("--playlist-end".to_string());
        match channel.playlist_end {
            Some(end) => args.push(end.to_string()),
            None => args.push("100".to_string()),
        }
    }

    args.push(channel_id_to_url(&channel.channel_id));

    args
}

fn directory_exists(path: &impl AsRef<Path>) -> bool {
    match fs::metadata(path) {
        Err(_) => false,
        Ok(m) => m.is_dir(),
    }
}

fn channel_id_to_url(channel_id: &str) -> String {
    format!("https://www.youtube.com/channel/{}", channel_id)
}
