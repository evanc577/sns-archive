use std::{fmt::Display, error::Error};

#[derive(Debug)]
pub enum TikTokError {
    TikTokHttp,
    Html,
    Snaptik(String),
}

impl Display for TikTokError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TikTokHttp => write!(f, "unable to fetch Tik Tok web page"),
            Self::Html => write!(f, "unable to parse Tik Tok HTML"),
            Self::Snaptik(s) => write!(f, "unable to use Snaptik: {}", s),
        }
    }
}

impl Error for TikTokError {}
