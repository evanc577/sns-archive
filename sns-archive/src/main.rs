use std::path::PathBuf;
use std::process;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use sns_archive::config::Config;

/// Archive various social networking services
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Services to archive
    #[clap(subcommand)] sns: Sns,

    /// Config file location
    #[arg(short, long, default_value_os_t = default_config_path())]
    config: PathBuf,
}

#[derive(Subcommand, Debug)]
enum Sns {
    /// Download Weverse posts and moments
    #[command(verbatim_doc_comment)]
    Weverse,
    /// Download Youtube videos
    Youtube,
    /// Download Naver Blog post images
    NaverPost,
    /// (Experimental) Download Xiao Hong Shu images and videos
    XiaoHongShu {
        /// API response from XiaoHongShu app from endpoint
        /// https://edith.xiaohongshu.com/api/sns/v4/note/user/posted
        /// Need to sniff app traffic
        json_file: PathBuf,
    },
    /// Download Weibo user posts
    Weibo,
    /// Download TikTok videos
    Tiktok {
        /// tiktok page html
        #[arg(short, long)]
        input_file: Option<PathBuf>,
    },
}

fn default_config_path() -> PathBuf {
    directories::ProjectDirs::from("", "", "SNS Archive")
        .unwrap()
        .config_dir()
        .join("config.toml")
}

#[tokio::main]
async fn main() {
    match run().await {
        Ok(_) => process::exit(0),
        Err(err) => {
            eprintln!("{}", err);
            process::exit(1);
        }
    }
}

async fn run() -> Result<()> {
    let args = Args::parse();
    let conf = Config::read(args.config)?;

    match args.sns {
        Sns::Weverse => {
            if let Some(conf) = conf.weverse {
                sns_archive::weverse::download(conf).await?;
            } else {
                return Err(anyhow!("Missing weverse section in config file"));
            }
        }
        Sns::Youtube => {
            if let Some(conf) = conf.youtube {
                sns_archive::youtube::download(conf)?;
            } else {
                return Err(anyhow!("Missing youtube section in config file"));
            }
        }
        Sns::NaverPost => {
            if let Some(conf) = conf.naver_post {
                sns_archive::naver_post::download_members(conf.members).await?;
            } else {
                return Err(anyhow!("Missing naver post section in config file"));
            }
        }
        Sns::XiaoHongShu { json_file: f } => {
            if let Some(conf) = conf.xiaohongshu {
                sns_archive::xiaohongshu::download(f, conf).await?;
            } else {
                return Err(anyhow!("Missing xiaohongshu section in config file"));
            }
        }
        Sns::Weibo => {
            if let Some(conf) = conf.weibo {
                sns_archive::weibo::download(conf).await?;
            } else {
                return Err(anyhow!("Missing weibo section in config file"));
            }
        }
        Sns::Tiktok { input_file } => {
            if let Some(conf) = conf.tiktok {
                if let Some(input) = input_file {
                    sns_archive::tiktok::download_from_html(input).await?;
                } else {
                    sns_archive::tiktok::download(conf).await?;
                }
            } else {
                return Err(anyhow!("Missing tiktok section in config file"));
            }
        }
    }

    Ok(())
}
