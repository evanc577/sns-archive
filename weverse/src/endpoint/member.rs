use serde::{Deserialize, Serialize};

use super::community_id::CommunityId;

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    #[serde(rename = "memberId")]
    pub id: String,
    pub community_id: CommunityId,
    pub profile_name: String,
    pub profile_type: String,
    #[serde(rename = "artistOfficialProfile")]
    pub official_profile: OfficialProfile,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OfficialProfile {
    pub official_name: String,
}
