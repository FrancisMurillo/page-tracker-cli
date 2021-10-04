# Page Tracker CLI

A simple Rust CLI tool to download Cloudflare KV data into a timestamped
CSV file for page view analytics for [fnlog.dev](https://fnlog.dev).

This can be run for a generic KV with string keys and positive integer
values. Make sure to get your [API
Token](https://dash.cloudflare.com/profile/api-tokens)(`$PT_JWT`) and
account and kv id pair by matching this URL in the KV dashboard:
`https://dash.cloudflare.com/$PT_ACCOUNT_ID/workers/kv/namespaces/$PT_KV_ID`.

To run this:

```shell
$ export PT_JWT="cloudflare_jwt" PT_ACCOUNT_ID="account_id"
PT_KV_ID="kv_id"

$ mkdir data
$ cargo run -- download --output-dir "./data"

$ cat ./data/*.csv
```

```
