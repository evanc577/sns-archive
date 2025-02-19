use std::path::Path;
use std::sync::LazyLock;
use std::time::Duration;

use futures::StreamExt;
use reqwest::Url;
use tokio::io::AsyncWriteExt;

use crate::util::{parse_date, slug, NaverBlogMetadata};
use crate::{ImageType, NaverBlogClient, NaverBlogError, ProgressBar};

pub(crate) enum NaverBlogDownloadStatus {
    Downloaded,
    Exists,
}

impl NaverBlogClient<'_> {
    pub(crate) async fn download_post<PB: ProgressBar>(
        &self,
        download_path: impl AsRef<Path>,
        member: &str,
        id: u64,
        image_type: ImageType,
    ) -> Result<NaverBlogDownloadStatus, NaverBlogError> {
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
        let slug = slug(&metadata);
        let blog_post_url = format!("https://blog.naver.com/{member}/{id}");

        // Skip if the blog post has already been downloaded
        if tokio::fs::metadata(download_path.as_ref().join(&slug))
            .await
            .is_ok()
        {
            return Ok(NaverBlogDownloadStatus::Exists);
        }

        // Download blog post
        eprintln!("Downloading {}", &blog_post_url);
        let images = extract_images(&document, image_type);

        // Progress bar
        let pb = PB::init(images.len(), &blog_post_url);

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
        // On windows, sometimes moving the directory fails with OS err 5 for some reason,
        // retry a few times before giving up
        const ATTEMPTS: usize = 10;
        let final_dir_path = download_path.as_ref().join(slug);
        for i in 0.. {
            let rename_res = tokio::fs::rename(&tmp_dir_path, &final_dir_path)
                .await
                .map_err(|e| NaverBlogError::DownloadBlogPost {
                    blog_post_url: blog_post_url.clone(),
                    msg: e.to_string(),
                });

            match rename_res {
                Ok(_) => {
                    break;
                }
                Err(err) => {
                    // Give up if reached max attempts
                    if i + 1 == ATTEMPTS {
                        return Err(err);
                    }

                    // Sleep for a bit and try again
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }

        pb.destroy();

        Ok(NaverBlogDownloadStatus::Downloaded)
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

fn extract_images(document: &scraper::Html, image_type: ImageType) -> Vec<String> {
    static SELECTOR: LazyLock<scraper::Selector> = LazyLock::new(|| {
        scraper::Selector::parse(".se-main-container .se-module-image-link > img").unwrap()
    });

    document
        .select(&SELECTOR)
        .filter_map(|element| element.value().attr("src"))
        .filter_map(|s| {
            // Change the query parameter to get the high res / original version
            let mut url = Url::parse(s).ok()?;
            if ImageType::is_handled(&url) {
                url = Url::parse(&format!(
                    "{}://{}{}",
                    image_type.protocol(),
                    image_type.domain(),
                    url.path()
                ))
                .unwrap();
                url.set_query(image_type.query());
            } else {
                eprintln!("INFO: external image: {:?}", url.as_str());
            }
            Some(url.to_string())
        })
        .collect()
}

struct DownloadImageError {
    image_url: String,
    msg: String,
}

async fn download_images<PB: ProgressBar>(
    client: &reqwest::Client,
    download_dir: impl AsRef<Path>,
    urls: &[String],
    slug: &str,
    pb: &PB,
) -> Result<(), DownloadImageError> {
    // Helper function to download a single image
    async fn download_one_image<PB: ProgressBar>(
        client: &reqwest::Client,
        base_dir: impl AsRef<Path>,
        index: usize,
        slug: &str,
        url: String,
        pb: &PB,
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

        pb.increment();

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
