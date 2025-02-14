use thiserror::Error;

#[derive(Error, Debug)]
pub enum NaverBlogError {
    #[error("invalid url: {url}")]
    InvalidUrl { url: String },

    #[error("unable to fetch blog post: {url}, {error:#?}")]
    FetchBlogPost {
        url: String,
        error: reqwest::Error,
    },

    #[error("unable to parse blog post: {url}, {msg}")]
    ParseBlogPost {
        url: String,
        msg: String,
    },

    #[error("unable to download image: blog post: {blog_post_url}, image: {image_url}, {msg}")]
    DownloadImage {
        blog_post_url: String,
        image_url: String,
        msg: String,
    },

    #[error("unable to download blog post: {blog_post_url}, {msg}")]
    DownloadBlogPost {
        blog_post_url: String,
        msg: String,
    },
}
