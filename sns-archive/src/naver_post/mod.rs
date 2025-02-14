use anyhow::Result;
use naver_blog::NaverBlogClient;

use crate::config::naver_post::NPMember;

pub async fn download_members(members: Vec<NPMember>) -> Result<()> {
    let reqwest_client = reqwest::Client::new();
    let client = NaverBlogClient::new(&reqwest_client);
    for member in members {
        client
            .download_member::<ProgressBar>(
                &member.id,
                member.download_path,
                member.filter.as_ref(),
                member.limit,
            )
            .await?;
    }

    Ok(())
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
