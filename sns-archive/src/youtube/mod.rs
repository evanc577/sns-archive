use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Result};

use crate::config::youtube::{YTChannel, YoutubeConfig};

//fn main() -> Result<(), ()> {
//let config = read_config();

//download(config.channels.iter(), config.filter.as_str())?;

//Ok(())
//}

pub fn download(config: YoutubeConfig) -> Result<()> {
    let channels = config.channels;
    let filter = config.filter;
    for channel in channels {
        // Check if new channel
        let new_channel = !directory_exists(&config.download_path.join(&channel.display_name));
        let tmp_dir = config
            .download_path
            .join(format!(".{}.temp", &channel.display_name));
        let final_dir = config.download_path.join(&channel.display_name);
        let target_dir = match new_channel {
            true => {
                fs::create_dir_all(&tmp_dir).unwrap();
                &tmp_dir
            }
            false => &final_dir,
        };

        // Build and run yt-dl command
        let args = generate_cmd_args(&channel, &target_dir, &filter, new_channel)?;
        let output = Command::new("yt-dlp")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .args(&args)
            .output()
            .expect("Failed to execute command");
        if !output.status.success() {
            eprintln!("{}", output.status);
            return Err(anyhow!("yt-dlp failed"));
        }

        if new_channel {
            fs::rename(&tmp_dir, &final_dir).unwrap_or_else(|_| {
                panic!(
                    "Could not rename directory {:?} to {:?}",
                    &tmp_dir, &final_dir
                )
            });
        }
    }

    Ok(())
}

fn generate_cmd_args(
    channel: &YTChannel,
    target_dir: impl AsRef<Path>,
    default_filter: &str,
    new_channel: bool,
) -> Result<Vec<OsString>> {
    let mut args = vec![
        OsString::from("--format-sort"),
        "res,fps,vcodec,acodec".into(),
        "--ignore-config".into(),
        "--verbose".into(),
        "--all-subs".into(),
        "--embed-subs".into(),
        "--compat-options".into(),
        "no-live-chat".into(),
        "--ignore-errors".into(),
        "--match-filter".into(),
        "!is_live".into(),
        "--remux-video".into(),
        "mkv".into(),
        "--output".into(),
        target_dir
            .as_ref()
            .join("%(upload_date)s_%(title)s_%(id)s.%(ext)s")
            .into(),
    ];

    if channel.apply_filter {
        args.push("--match-title".into());
        match &channel.custom_filter {
            Some(f) => args.push(f.into()),
            None => args.push(default_filter.into()),
        }
    }

    if !channel.always_redownload {
        args.push("--download-archive".into());
        args.push(
            target_dir
                .as_ref()
                .parent()
                .unwrap()
                .join("downloaded.txt")
                .into(),
        );
    }

    if !new_channel {
        args.push("--playlist-end".into());
        match channel.playlist_end {
            Some(end) => args.push(end.to_string().into()),
            None => args.push("100".into()),
        }
    }

    args.push(channel_id_to_url(&channel.channel_id).into());

    Ok(args)
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
