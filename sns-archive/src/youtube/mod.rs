use std::ffi::OsString;
use std::fmt::Display;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::{fs, mem};

use once_cell::sync::Lazy;
use regex::Regex;

use crate::config::youtube::{YTChannel, YoutubeConfig};

#[derive(Debug)]
pub struct YTError(Vec<YTChannel>);

impl Display for YTError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "failed to download {} channels:", self.0.len())?;
        for channel in &self.0 {
            writeln!(f, "  {} ({})", channel.channel_id, channel.display_name)?;
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
        let dir = config.download_path.join(&channel.display_name);
        let new_channel = !directory_exists(&dir);
        let tmp_dir = {
            let mut temp_str = dir.as_os_str().to_owned();
            temp_str.push(".temp");
            PathBuf::from(temp_str)
        };
        let target_dir = match new_channel {
            true => &tmp_dir,
            false => &dir,
        };

        // Build and run yt-dl command
        let args = generate_cmd_args(
            &channel,
            target_dir,
            &filter,
            new_channel,
            &config.archive_path,
        );
        let mut child = Command::new("yt-dlp")
            .stdout(Stdio::inherit())
            .stderr(Stdio::piped())
            .args(&args)
            .spawn()
            .expect("Failed to execute command");

        // Tee stderr to in memory buffer to inspect any errors
        let child_err = mem::take(&mut child.stderr).unwrap();
        let stderr = std::thread::spawn(move || tee(child_err, std::io::stderr()).unwrap())
            .join()
            .unwrap();

        let status = child.wait().unwrap();
        if !status.success() && !error_is_geo_restrict(&String::from_utf8(stderr).unwrap()) {
            eprintln!("{}", status);
            errored_channels.push(channel);
            continue;
        }

        if new_channel && directory_exists(&tmp_dir) {
            fs::rename(&tmp_dir, &dir).unwrap_or_else(|_| {
                panic!("Could not rename directory {:?} to {:?}", &tmp_dir, &dir)
            });
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
    target_dir: impl AsRef<Path>,
    default_filter: &str,
    new_channel: bool,
    archive_path: impl AsRef<Path>,
) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("--format-sort"),
        "res,fps,vcodec,acodec".into(),
        "--ignore-config".into(),
        "--all-subs".into(),
        "--embed-subs".into(),
        "--compat-options".into(),
        "no-live-chat".into(),
        "--ignore-errors".into(),
        "--match-filter".into(),
        "live_status != is_live & live_status != is_upcoming".into(),
        "--remux-video".into(),
        "mkv".into(),
        "--output".into(),
        target_dir
            .as_ref()
            .join("%(upload_date)s_%(title)s_%(id)s.%(ext)s")
            .as_os_str()
            .to_owned(),
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
        args.push(archive_path.as_ref().as_os_str().to_owned());
    }

    if !new_channel {
        args.push("--playlist-end".into());
        match channel.playlist_end {
            Some(end) => args.push(end.to_string().into()),
            None => args.push("100".into()),
        }
    }

    args.push(channel_id_to_url(&channel.channel_id).into());

    args
}

fn tee(mut stream: impl Read, mut output_2: impl Write) -> std::io::Result<Vec<u8>> {
    let mut buf = [0u8; 1024];
    let mut output = Vec::new();
    loop {
        let num_read = stream.read(&mut buf)?;
        if num_read == 0 {
            break;
        }

        let buf = &buf[..num_read];
        output.write_all(buf)?;
        output_2.write_all(buf)?;
    }

    Ok(output)
}

fn directory_exists(path: &impl AsRef<Path>) -> bool {
    match fs::metadata(path) {
        Err(_) => false,
        Ok(m) => m.is_dir(),
    }
}

fn channel_id_to_url(channel_id: &str) -> String {
    format!(
        "https://www.youtube.com/playlist?list=UU{}",
        &channel_id[2..]
    )
}

fn error_is_geo_restrict(log: &str) -> bool {
    static ERROR_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^ERROR:").unwrap());
    static ERROR_MSG_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"The uploader has not made this video available in your country").unwrap()
    });
    0 == log
        .lines()
        .filter(|line| ERROR_RE.find(line.trim()).is_some())
        .filter(|line| ERROR_MSG_RE.find(line).is_none())
        .count()
}
