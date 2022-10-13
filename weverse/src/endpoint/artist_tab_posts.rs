use anyhow::Result;
use futures::Stream;
use reqwest::Client;

use super::community_id::CommunityId;
use super::post::{post, ArtistPost};

pub(crate) struct ArtistPosts {
    all_ids: Vec<String>,
    current_idx: usize,
}

impl ArtistPosts {
    pub(crate) async fn as_stream<'a>(
        &'a mut self,
        client: &'a Client,
        auth: &'a str,
        community_id: CommunityId,
    ) -> impl Stream<Item = Result<ArtistPost>> + 'a {
        futures::stream::unfold(self, |state| async {
            let post_id = &state.all_ids[state.current_idx];
            let post = post(client, auth, &post_id).await;
            Some((post, state))
        })
    }
}
