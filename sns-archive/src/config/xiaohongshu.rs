use std::path::PathBuf;

use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct XiaoHongShuConfig {
    #[serde(deserialize_with = "super::deserialize_path")]
    pub download_path: PathBuf,
}
