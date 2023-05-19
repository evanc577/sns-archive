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
    /// Download Twitter tweets
    Twitter {
        /// Read tweets to save from input file, 1 tweet ID per line
        #[clap(short, long, value_parser)]
        input: Option<PathBuf>,

        /// Only save tweets from user
        #[clap(short, long, value_parser)]
        filter: Option<String>,
    },
    /// Download Weverse posts and moments
    ///
    /// Example config.toml section:
    ///
    /// [weverse]
    /// # Weverse login information
    /// email = ""
    /// password = ""
    /// # Maximum number of concurrent network connections (optional, default 20)
    /// max_connections = {integer}
    ///
    /// # Section for each artist to archive
    /// [weverse.artists.dreamcatcher]
    /// # Directory to save artist posts (optional, don't download posts if none)
    /// artist_download_path = "path/to/directory"
    /// # Directory to save artist moments (optiona, don't download moments if none)
    /// moments_download_path = "path/to/directory"
    #[clap(verbatim_doc_comment)]
    Weverse,
    /// Download Youtube videos
    Youtube,
    /// Download Naver post images
    NaverPost,
    /// (Experimental) Download Xiao Hong Shu images and videos
    XiaoHongShu {
        /// API response from XiaoHongShu app from endpoint
        /// https://edith.xiaohongshu.com/api/sns/v4/note/user/posted
        /// Need to sniff app traffic
        #[clap(value_parser)]
        json_file: PathBuf,
    },
    /// Download Weibo user posts
    Weibo,
    /// Download TikTok videos
    Tiktok {
        /// tiktok page html
        #[clap(short, long, value_parser)]
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
