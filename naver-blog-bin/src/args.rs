use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueHint};
use naver_blog::NaverBlogClient;
use regex::RegexBuilder;

#[derive(Parser, Debug)]
pub struct Args {
    #[arg(value_hint = ValueHint::DirPath)]
    download_path: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Download a specific Naver Blog post with its URL
    Url {
        /// URL of Naver Blog post
        url: String,
    },

    /// Download multiple Naver Blog posts
    Member {
        blog_id: String,

        /// Limit number of blog posts download
        #[arg(short, long)]
        limit: Option<usize>,

        /// Only download blog posts matching this Regex
        #[arg(short, long)]
        filter: Option<String>,
    },
}

impl Args {
    pub async fn download(&self) -> Result<()> {
        let reqwest_client = reqwest::Client::new();
        let client = NaverBlogClient::new(&reqwest_client);

        match &self.command {
            Commands::Url { url } => {
                client.download_url(url, &self.download_path).await?;
            }
            Commands::Member {
                blog_id,
                limit,
                filter,
            } => {
                let filter = filter
                    .as_ref()
                    .map(|f| RegexBuilder::new(f).case_insensitive(true).build())
                    .transpose()?;
                client
                    .download_member(blog_id, &self.download_path, filter.as_ref(), *limit)
                    .await?;
            }
        }

        Ok(())
    }
}
