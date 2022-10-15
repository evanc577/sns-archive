use std::path::PathBuf;
use std::process;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use sns_archive::config::Config;

/// Archive various social networking services
#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Services to archive
    #[clap(subcommand)]
    sns: Sns,

    /// Config file location
    #[clap(short, long, default_value_os_t = default_config_path(), value_parser)]
    config: PathBuf,
}

#[derive(Subcommand, Debug)]
enum Sns {
    Twitter {
        /// Read tweets to save from input file, 1 tweet ID per line
        #[clap(short, long, value_parser)]
        input: Option<PathBuf>,

        /// Only save tweets from user
        #[clap(short, long, value_parser)]
        filter: Option<String>,
    },
    Weverse,
    Youtube,
    NaverPost,
    XiaoHongShu {
        /// API response from XiaoHongShu app from endpoint
        /// https://edith.xiaohongshu.com/api/sns/v4/note/user/posted
        /// Need to sniff app traffic
        #[clap(value_parser)]
        json_file: PathBuf,
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
            todo!()
        }
        Sns::Twitter {
            input: i,
            filter: f,
        } => {
            if let Some(conf) = conf.twitter {
                sns_archive::twitter::download(conf, i.as_deref(), f.as_deref()).await?;
            } else {
                return Err(anyhow!("Missing twitter section in config file"));
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
    }

    Ok(())
}
