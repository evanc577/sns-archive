use std::path::PathBuf;

use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct XiaoHongShuConfig {
    #[serde(deserialize_with = "super::deserialize_directory")]
    pub download_path: PathBuf,
}
