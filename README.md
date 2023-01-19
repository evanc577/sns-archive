# sns-archive

Unified SNS archiver

Currently supports:

* Twitter
* Weverse
* Youtube
* Naver post
* Xiaohongshu (experimental, requires json from app)

## Usage

Create a config file `$XDG_CONFIG_DIR/snsarchive/config.toml`

Sample:

```toml
[weverse]
email = "" # Weverse email
password = "" # Weverse password

[weverse.artists.dreamcatcher]
artist_download_path = "sns/weverse/artist"
moments_download_path = "sns/weverse/moments"

[twitter]
bearer = "" # Twitter API bearer token
download_path = "sns/twitter"
timezone_offset = 32400
users = ["hf_dreamcatcher"]

[youtube]
download_path = "sns/youtube"
archive_path = "sns/youtube/downloaded.txt"
filter = "dreamcatcher|(dream catcher)|드림캐쳐"
channels = [
  { channel_id = "UCxGkExhl-tIwOt7E-DoVJWg", display_name = "seezn", enabled = false, apply_filter = true },
  { channel_id = "UCwnBKt1bJfKXGH2Q1IaTnAw", display_name = "e.L.e", apply_filter = true },
]

[[naver_post.members]]
id = "29156514"
download_path = "sns/naver_post/dreamcatcher_company"
limit = 5

[xiaohongshu]
download_path = "sns/xiaohongshu"
```
