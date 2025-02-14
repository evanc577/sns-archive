use anyhow::Result;
use clap::Parser;
use naver_blog_bin::Args;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    args.download().await
}
