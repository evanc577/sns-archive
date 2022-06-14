# sns-archive

Unified SNS archiver

Currently supports:

* Twitter
* Weverse
* Youtube

## Usage

Create a config file `$XDG_CONFIG_DIR/snsarchive/config.toml`

Sample:

```toml
[weverse]
cookies_file = "" # Cookies file for weverse.io in netscape format
max_connections = 20

[weverse.artists.dreamcatcher]
artist_download_path = "sns/weverse/artist"
moments_download_path = "sns/weverse/moments"
videos_download_path = "sns/weverse/videos"
recent_artist = 10
recent_moments = 10

[twitter]
bearer = "" # Twitter API bearer token
download_path = "sns/twitter"
timezone_offset = 32400
users = [
    "hf_dreamcatcher",
]

[youtube]
download_path = "sns/youtube"
filter = "dreamcatcher|(dream catcher)|드림캐쳐"
channels = [
    { channel_id = "UCxGkExhl-tIwOt7E-DoVJWg", display_name = "seezn", apply_filter = true },
]
```
