use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Deserialize)]
struct Config {
    download_all: Vec<YTChannel>,
    filter: String,
    download_filter: Vec<YTChannel>,
}

#[derive(Deserialize)]
struct YTChannel {
    channel_id: String,
    display_name: String,
}

fn main() -> Result<(), ()> {
    let config = read_config();

    download(config.download_filter.iter(), Some(&config.filter))?;
    download(config.download_all.iter(), None)?;

    Ok(())
}

fn read_config() -> Config {
    let path = "config.toml";
    let data = std::fs::read_to_string(path).expect(&format!("Unable to read {}", path));
    toml::from_str(&data).expect(&format!("Unable to parse {}", path))
}

fn download<'a>(
    channels: impl Iterator<Item = &'a YTChannel>,
    filter: Option<&str>,
) -> Result<(), ()> {
    for channel in channels {
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
        let args = match filter {
            Some(f) => process_channel_filter(&channel, &target_dir, &f, new_channel)?,
            None => process_channel_all(&channel, &target_dir)?,
        };
        let output = Command::new("yt-dlp")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .args(&args)
            .output()
            .expect("Failed to execute command");
        if !output.status.success() {
            eprintln!("{}", output.status);
            return Err(());
        }

        if new_channel {
            fs::rename(&tmp_dir, &channel.display_name).expect(&format!(
                "Could not rename directory {} to {}",
                &tmp_dir, &channel.display_name
            ));
        }
    }

    Ok(())
}

fn process_channel_filter(
    channel: &YTChannel,
    target_dir: &str,
    filter: &str,
    new_channel: bool,
) -> Result<Vec<String>, ()> {
    let playlist_end = match new_channel {
        true => "1000000000",
        false => "100",
    };

    let args = vec![
        "--format-sort".to_string(),
        "res,fps,vcodec,acodec".to_string(),
        "--ignore-config".to_string(),
        "--verbose".to_string(),
        "--all-subs".to_string(),
        "--embed-subs".to_string(),
        "--compat-options".to_string(),
        "no-live-chat".to_string(),
        "--ignore-errors".to_string(),
        "--download-archive".to_string(),
        "downloaded.txt".to_string(),
        "--match-title".to_string(),
        filter.to_owned(),
        "--playlist-end".to_string(),
        playlist_end.to_owned(),
        "--remux-video".to_string(),
        "mkv".to_string(),
        "--output".to_string(),
        Path::new(&target_dir)
            .join("%(upload_date)s_%(title)s_%(id)s.%(ext)s")
            .to_str()
            .unwrap()
            .to_owned(),
        channel_id_to_url(&channel.channel_id),
    ];

    Ok(args)
}

fn process_channel_all(channel: &YTChannel, target_dir: &str) -> Result<Vec<String>, ()> {
    let args = vec![
        "--format-sort".to_string(),
        "res,fps,vcodec,acodec".to_string(),
        "--ignore-config".to_string(),
        "--verbose".to_string(),
        "--all-subs".to_string(),
        "--embed-subs".to_string(),
        "--compat-options".to_string(),
        "no-live-chat".to_string(),
        "--ignore-errors".to_string(),
        "--playlist-end".to_string(),
        "50".to_string(),
        "--remux-video".to_string(),
        "mkv".to_string(),
        "--output".to_string(),
        Path::new(&target_dir)
            .join("%(upload_date)s_%(title)s_%(id)s.%(ext)s")
            .to_str()
            .unwrap()
            .to_owned(),
        channel_id_to_url(&channel.channel_id),
    ];

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
