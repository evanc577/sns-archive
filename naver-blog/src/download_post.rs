use std::path::Path;
use std::sync::LazyLock;

use futures::StreamExt;
use reqwest::Url;
use tokio::io::AsyncWriteExt;

use crate::util::{parse_date, slug, NaverBlogMetadata};
use crate::{NaverBlogClient, NaverBlogError};

impl NaverBlogClient<'_> {
    pub(crate) async fn download_post(
        &self,
        download_path: impl AsRef<Path>,
        member: &str,
        id: u64,
    ) -> Result<(), NaverBlogError> {
        // Get the page HTML
        let mut url = Url::parse("https://blog.naver.com/PostView.naver").unwrap();
        url.query_pairs_mut()
            .append_pair("blogId", member)
            .append_pair("logNo", &id.to_string());

        let err_func = |e| -> _ {
            NaverBlogError::FetchBlogPost {
                url: url.to_string(),
                error: e,
            }
        };
        let html = self
            .client
            .get(url.clone())
            .send()
            .await
            .map_err(err_func)?
            .error_for_status()
            .map_err(err_func)?
            .text()
            .await
            .map_err(err_func)?;

        // Extract images from page source
        let document = scraper::Html::parse_document(&html);
        let metadata = extract_post_metadata(&document, member.to_owned(), id).map_err(|msg| {
            NaverBlogError::ParseBlogPost {
                url: url.to_string(),
                msg,
            }
        })?;
        let images = extract_images(&document);
        let slug = slug(&metadata);
        let blog_post_url = format!("https://blog.naver.com/{member}/{id}");

        // Skip if the blog post has already been downloaded
        if tokio::fs::metadata(download_path.as_ref().join(&slug))
            .await
            .is_ok()
        {
            return Ok(());
        }

        // Download blog post
        eprintln!("Downloading {}", &blog_post_url);
        let pb = indicatif::ProgressBar::new(images.len() as u64);
        let sty = indicatif::ProgressStyle::default_bar()
            .template("[{wide_bar}] {pos:>3}/{len:3}")
            .unwrap()
            .progress_chars("=> ");
        pb.set_style(sty);

        // Create temporary directory to download images to
        let tmp_dir_path = download_path.as_ref().join(format!(".tmp.{slug}"));
        tokio::fs::create_dir_all(&tmp_dir_path)
            .await
            .map_err(|e| NaverBlogError::DownloadBlogPost {
                blog_post_url: blog_post_url.clone(),
                msg: e.to_string(),
            })?;

        // Download images
        download_images(self.client, &tmp_dir_path, &images[..], &slug, &pb)
            .await
            .map_err(|e| NaverBlogError::DownloadImage {
                blog_post_url: url.to_string(),
                image_url: e.image_url,
                msg: e.msg,
            })?;

        // Move tmp dir to final location
        let final_dir_path = download_path.as_ref().join(slug);
        tokio::fs::rename(tmp_dir_path, final_dir_path)
            .await
            .map_err(|e| NaverBlogError::DownloadBlogPost {
                blog_post_url: blog_post_url.clone(),
                msg: e.to_string(),
            })?;

        pb.finish_and_clear();
        eprintln!("Downloaded {}", &blog_post_url);

        Ok(())
    }
}

fn extract_post_metadata(
    document: &scraper::Html,
    member: String,
    post_id: u64,
) -> Result<NaverBlogMetadata, String> {
    // Extract post title
    static TITLE_SELECTOR: LazyLock<scraper::Selector> =
        LazyLock::new(|| scraper::Selector::parse("meta[property=\"og:title\"]").unwrap());
    let title = document
        .select(&TITLE_SELECTOR)
        .next()
        .map(|e| e.value())
        .and_then(|v| v.attr("content"))
        .ok_or("failed to extract title".to_owned())?;

    // Extract date
    static DATE_SELECTOR: LazyLock<scraper::Selector> =
        LazyLock::new(|| scraper::Selector::parse(".se_publishDate").unwrap());
    let time = document
        .select(&DATE_SELECTOR)
        .next()
        .and_then(|e| e.text().next())
        .ok_or("failed to extract date".to_owned())?;
    let time = parse_date(time)?;

    Ok(NaverBlogMetadata {
        title: title.to_owned(),
        time,
        member,
        post_id,
    })
}

fn extract_images(document: &scraper::Html) -> Vec<String> {
    static SELECTOR: LazyLock<scraper::Selector> = LazyLock::new(|| {
        scraper::Selector::parse(".se-main-container .se-module-image-link > img").unwrap()
    });
    static FULL_RES_TYPE: &str = "type=w3840";

    document
        .select(&SELECTOR)
        .filter_map(|element| element.value().attr("src"))
        .filter_map(|s| {
            // Change the query parameter to get the high res / original version
            let mut url = Url::parse(s).ok()?;
            url.set_query(Some(FULL_RES_TYPE));
            Some(url.to_string())
        })
        .collect()
}

struct DownloadImageError {
    image_url: String,
    msg: String,
}

async fn download_images(
    client: &reqwest::Client,
    download_dir: impl AsRef<Path>,
    urls: &[String],
    slug: &str,
    pb: &indicatif::ProgressBar,
) -> Result<(), DownloadImageError> {
    // Helper function to download a single image
    async fn download_one_image(
        client: &reqwest::Client,
        base_dir: impl AsRef<Path>,
        index: usize,
        slug: &str,
        url: String,
        pb: &indicatif::ProgressBar,
    ) -> Result<(), DownloadImageError> {
        // Download to a temp file without extension first
        let err_func = |e: reqwest::Error| -> _ {
            DownloadImageError {
                image_url: url.clone(),
                msg: e.to_string(),
            }
        };
        let bytes = client
            .get(&url)
            .header(reqwest::header::REFERER, "https://blog.naver.com/")
            .send()
            .await
            .map_err(err_func)?
            .error_for_status()
            .map_err(err_func)?
            .bytes()
            .await
            .map_err(err_func)?;

        // Guess the extension from the file contents
        let ext = infer::get(&bytes)
            .map(|mime| mime.extension())
            .unwrap_or("jpg");

        // Write to destination
        let file = base_dir
            .as_ref()
            .join(format!("{}-img{:03}.{}", slug, index + 1, ext));
        let err_func = |e: std::io::Error| -> _ {
            DownloadImageError {
                image_url: url.clone(),
                msg: e.to_string(),
            }
        };
        let mut file = tokio::fs::File::create(file).await.map_err(err_func)?;
        file.write_all(&bytes).await.map_err(err_func)?;

        pb.inc(1);

        Ok(())
    }

    // Download images concurrently
    futures::stream::iter(urls.iter().enumerate().map(|(i, url)| {
        download_one_image(client, download_dir.as_ref(), i, slug, url.clone(), pb)
    }))
    .buffer_unordered(20)
    .collect::<Vec<_>>()
    .await
    .into_iter()
    .collect::<Result<(), _>>()?;

    Ok(())
}
