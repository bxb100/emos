Rust client for the Douban Frodo API. The client implementation is based on
[MoviePilot](https://github.com/jxxghp/MoviePilot/blob/v2/app/modules/douban/apiv2.py).

Create a `douban_client::Client` with `Client::new()`. Public resource modules are
`collections` for collection pages, `interests` for user interests, and `subject`
for shared subject types. JSON field names and API endpoint names follow Douban.
