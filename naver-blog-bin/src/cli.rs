use anyhow::Result;
use clap::Parser;
use naver_blog_bin::Args;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    args.download::<ProgressBar>().await
}

struct ProgressBar(indicatif::ProgressBar);

impl naver_blog::ProgressBar for ProgressBar {
    fn init(total: usize, _description: &str) -> Self {
        let pb = indicatif::ProgressBar::new(total as u64);
        let sty = indicatif::ProgressStyle::default_bar()
            .template("[{wide_bar}] {pos:>3}/{len:3}")
            .unwrap()
            .progress_chars("=> ");
        pb.set_style(sty);
        Self(pb)
    }

    fn increment(&self) {
        self.0.inc(1);
    }

    fn destroy(self) {
        self.0.finish_and_clear();
    }
}
