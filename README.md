# sns-archive

Unified SNS archiver

Currently supports:

* Weverse
* Youtube
* Naver blog
* Xiaohongshu (experimental, requires json from app)
* Weibo

## Usage

Create a config file `$XDG_CONFIG_DIR/snsarchive/config.toml`

Sample:

```toml
# Weverse
#
# email:    Weverse login email
# password: Weverse login password
# artists:  Table of Weverse artists
#
# Artist
#   artist_download_path:  (Optional) Path to download artist posts to
#   artist_download_limit: (Optional) Max number of posts to download
#   artist_stop_id:        (Optional) Stop downloading at this post ID
#   moments_download_path: (Optional) Path to download artist moments to
#   lives_download_path:   (Optional) Path to download artist lives to
#   lives_download_limit:  (Optional) Max number of lives to download
#   lives_stop_id:         (Optional) Stop downloading at this live ID
[weverse]
email = ""
password = ""

[weverse.artists.dreamcatcher]
artist_download_path = "sns/weverse/artist"
moments_download_path = "sns/weverse/moments"
lives_download_path = "sns/weverse/lives"

# Weibo
#
# users: List of Weibo users
#
# User
#   user:          Weibo user ID
#   download_path: Path to download files to
[weibo]
users = [
  { user = 7317173686, download_path = "sns/weibo/韩东_捕梦网" },
]

# YouTube
#
# download_path: Path to download files to
# archive_path:  File storing previously downloaded video IDs
# filter:        Default regex to filter video titles
# channels:      List of YouTube channels
#
# Channel
#   channel_id:        YouTube channel ID
#   display_name:      Used as download subdirectory
#   apply_filter:      (Optional) (Default: false) Apply the default regex filter
#   always_redownload: (Optional) (Default: false) Always redownload videos
#   custom_filter:     (Optional) Use a custom regex filter rather than the default filter
#   playlist_end:      (Optional) Max number of videos to download
#   enabled:           (Optional) (Default: true) Enable/disable downloading this channel
[youtube]
download_path = "sns/youtube"
archive_path = "sns/youtube/downloaded.txt"
filter = "dreamcatcher|(dream catcher)|드림캐쳐"
channels = [
  { channel_id = "UCxGkExhl-tIwOt7E-DoVJWg", display_name = "seezn", enabled = false, apply_filter = true },
  { channel_id = "UCwnBKt1bJfKXGH2Q1IaTnAw", display_name = "e.L.e", apply_filter = true },
]

# Naver Post
#
# members: List of Naver Post members
#
# Member
#   id:            Naver Post member ID
#   download_path: Path to download files to
#   limit:         (Optional) Maximum number of posts to check
[naver_post]
members = [
  { id = "29156514", download_path = "sns/naver_post/dreamcatcher_company", limit = 5},
]

[xiaohongshu]
download_path = "sns/xiaohongshu"
```
