use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum, ValueHint};
use naver_blog::{ImageType, NaverBlogClient, ProgressBar};
use regex::RegexBuilder;

#[derive(Parser, Debug)]
pub struct Args {
    /// Directory to download images to. A subdirectory will be created for each blog post.
    #[arg(value_hint = ValueHint::DirPath)]
    download_path: PathBuf,

    /// Resolution and type image to download.
    #[arg(value_enum, short, long, default_value_t = ArgImageType::WebpOriginal)]
    image_type: ArgImageType,

    #[command(subcommand)]
    command: Commands,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum ArgImageType {
    WebpOriginal,
    JpegW3840,
    JpegW966,
    JpegW800,
}

impl From<ArgImageType> for ImageType {
    fn from(value: ArgImageType) -> Self {
        match value {
            ArgImageType::WebpOriginal => ImageType::WebpOriginal,
            ArgImageType::JpegW3840 => ImageType::JpegW3840,
            ArgImageType::JpegW966 => ImageType::JpegW966,
            ArgImageType::JpegW800 => ImageType::JpegW800,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Download a specific Naver Blog post with its URL.
    Url {
        /// URL of Naver Blog post.
        url: String,
    },

    /// Download multiple Naver Blog posts.
    Member {
        /// Username of the blog author.
        blog_id: String,

        /// Limit number of blog posts processed. Filtered posts will count towards the limit.
        #[arg(short, long)]
        limit: Option<usize>,

        /// Only download blog posts matching this Regex.
        #[arg(short, long)]
        filter: Option<String>,

        /// Newest blog posts ID to download.
        #[arg(long)]
        until_post: Option<u64>,

        /// Oldest blog posts ID to download.
        #[arg(long)]
        since_post: Option<u64>,
    },
}

impl Args {
    pub async fn download<PB: ProgressBar>(&self) -> Result<()> {
        let reqwest_client = reqwest::Client::new();
        let client = NaverBlogClient::new(&reqwest_client);

        match &self.command {
            Commands::Url { url } => {
                client
                    .download_url::<PB>(url, &self.download_path, self.image_type.into())
                    .await?;
            }
            Commands::Member {
                blog_id,
                limit,
                filter,
                until_post,
                since_post,
            } => {
                let filter = filter
                    .as_ref()
                    .map(|f| RegexBuilder::new(f).case_insensitive(true).build())
                    .transpose()?;
                client
                    .download_member::<PB>(
                        blog_id,
                        &self.download_path,
                        filter.as_ref(),
                        *limit,
                        self.image_type.into(),
                        *until_post,
                        *since_post,
                    )
                    .await?;
            }
        }

        Ok(())
    }
}
