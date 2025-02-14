use std::sync::LazyLock;

use page_turner::{PageTurner, TurnedPage, TurnedPageResult};
use regex::Regex;
use serde::{Deserialize, Deserializer};

use crate::{NaverBlogClient, NaverBlogError};

#[derive(Clone)]
pub(crate) struct GetPostsRequest {
    member: String,
    page: usize,
}

impl GetPostsRequest {
    pub(crate) fn new(member: String) -> Self {
        Self { member, page: 1 }
    }
}

#[derive(Deserialize, Debug)]
pub(crate) struct NaverBlogPostStub {
    #[serde(rename = "logNo")]
    #[serde(deserialize_with = "parse_u64")]
    pub post_id: u64,
    pub title: String,
}

struct GetPostResponse {
    posts: Vec<NaverBlogPostStub>,
    more: bool,
}

impl PageTurner<GetPostsRequest> for NaverBlogClient<'_> {
    type PageItems = Vec<NaverBlogPostStub>;
    type PageError = NaverBlogError;

    async fn turn_page(
        &self,
        mut request: GetPostsRequest,
    ) -> TurnedPageResult<Self, GetPostsRequest> {
        let response = self.get_posts(request.clone()).await?;
        if response.more {
            request.page += 1;
            Ok(TurnedPage::next(response.posts, request))
        } else {
            Ok(TurnedPage::last(response.posts))
        }
    }
}

impl NaverBlogClient<'_> {
    async fn get_posts(&self, request: GetPostsRequest) -> Result<GetPostResponse, NaverBlogError> {
        const COUNT_PER_PAGE: usize = 30;

        let err_func = |e: reqwest::Error| NaverBlogError::FetchBlogPostList {
            member: request.member.clone(),
            page: request.page,
            msg: e.to_string(),
        };

        let text = self
            .client
            .get("https://blog.naver.com/PostTitleListAsync.naver")
            .query(&[
                ("blogId", request.member.as_str()),
                ("countPerPage", COUNT_PER_PAGE.to_string().as_str()),
                ("currentPage", request.page.to_string().as_str()),
            ])
            .send()
            .await
            .map_err(err_func)?
            .error_for_status()
            .map_err(err_func)?
            .text()
            .await
            .map_err(err_func)?;

        // Remove bad single quote escapes
        static QUOTE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\'").unwrap());
        let text = QUOTE_RE.replace_all(&text, "'");

        // Parse json
        #[derive(Deserialize, Debug)]
        #[serde(rename_all = "camelCase")]
        struct Response {
            post_list: Vec<NaverBlogPostStub>,
            #[serde(deserialize_with = "parse_u64")]
            total_count: u64,
        }

        let mut parsed: Response =
            serde_json::from_str(&text).map_err(|e| NaverBlogError::FetchBlogPostList {
                member: request.member.clone(),
                page: request.page,
                msg: e.to_string(),
            })?;

        // Unescape title
        for post in parsed.post_list.iter_mut() {
            // URL escape
            post.title = urlencoding::decode(&post.title)
                .map_err(|e| NaverBlogError::FetchBlogPostList {
                    member: request.member.clone(),
                    page: request.page,
                    msg: e.to_string(),
                })?
                .into_owned();

            // Decode "+" into space
            static PLUS_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\+").unwrap());
            post.title = PLUS_RE.replace_all(&post.title, " ").into_owned();

            // HTML escape
            post.title = htmlize::unescape(&post.title).into_owned();
        }

        Ok(GetPostResponse {
            posts: parsed.post_list,
            more: COUNT_PER_PAGE * request.page < parsed.total_count as usize,
        })
    }
}

fn parse_u64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    let s = String::deserialize(deserializer)?;
    s.parse().map_err(serde::de::Error::custom)
}
