use std::path::PathBuf;
use std::process;

use anyhow::Result;
use clap::{ArgEnum, Parser};
use sns_archive::config::Config;

/// Archive various social networking services
#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Services to archive
    #[clap(arg_enum, value_parser, short, long)]
    services: Vec<Sns>,

    /// Config file location
    #[clap(short, long, default_value_os_t = default_config_path())]
    config: PathBuf,
}

#[derive(ArgEnum, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Sns {
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

    for sns in args.services {
        match sns {
            Sns::Weverse => sns_archive::weverse::network::download(&conf.weverse)
                .await
                .map_err(|s| anyhow::anyhow!(s))?,
        }
    }

    Ok(())
}
