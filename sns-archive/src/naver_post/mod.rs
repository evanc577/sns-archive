use anyhow::Result;
use naver_blog::NaverBlogClient;

use crate::config::naver_post::NPMember;

pub async fn download_members(members: Vec<NPMember>) -> Result<()> {
    let reqwest_client = reqwest::Client::new();
    let client = NaverBlogClient::new(&reqwest_client);
    for member in members {
        client
            .download_member(
                &member.id,
                member.download_path,
                member.filter.as_ref(),
                member.limit,
            )
            .await?;
    }

    Ok(())
}
