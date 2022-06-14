use std::path::PathBuf;
use std::process;

use anyhow::Result;
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
            sns_archive::weverse::download(&conf.weverse)
                .await
                .map_err(|s| anyhow::anyhow!(s))?;
        }
        Sns::Twitter {
            input: i,
            filter: f,
        } => {
            sns_archive::twitter::download(conf.twitter, i.as_deref(), f.as_deref()).await?;
        }
    }

    Ok(())
}
