use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Deserialize)]
struct Config {
    channels: Vec<YTChannel>,
    filter: String,
}

#[derive(Deserialize)]
struct YTChannel {
    channel_id: String,
    display_name: String,
    #[serde(default)]
    apply_filter: bool,
    #[serde(default)]
    always_redownload: bool,
    custom_filter: Option<String>,
    playlist_end: Option<usize>,
}

fn main() -> Result<(), ()> {
    let config = read_config();

    download(config.channels.iter(), config.filter.as_str())?;

    Ok(())
}

fn read_config() -> Config {
    let path = "config.toml";
    let data = std::fs::read_to_string(path).expect(&format!("Unable to read {}", path));
    toml::from_str(&data).expect(&format!("Unable to parse {}", path))
}

fn download<'a>(
    channels: impl Iterator<Item = &'a YTChannel>,
    filter: &str,
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
        let args = generate_cmd_args(&channel, &target_dir, &filter, new_channel)?;
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

fn generate_cmd_args(
    channel: &YTChannel,
    target_dir: &str,
    default_filter: &str,
    new_channel: bool,
) -> Result<Vec<String>, ()> {

    let mut args = vec![
        "--format-sort".to_string(),
        "res,fps,vcodec,acodec".to_string(),
        "--ignore-config".to_string(),
        "--verbose".to_string(),
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
