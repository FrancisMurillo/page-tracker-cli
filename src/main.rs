use anyhow::Result;
use chrono::{DateTime, Utc};
use csv::Writer;
use env_logger::Env;
use futures::{
    future::FutureExt,
    stream::{FuturesUnordered, StreamExt},
};
use log::info;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{io::Write, path::PathBuf};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(about = "Page Tracker commands")]
/// A CLI to manage a simple Cloudflare KV page tracker.
///
/// View the desired page tracker KV and check the URL to match the
/// environment variables:
/// https://dash.cloudflare.com/$PT_ACCOUNT_ID/workers/kv/namespaces/$PT_KV_ID
///
/// Also create a JWT token with the permission `Account.Workers KV
/// Storage` for $PT_JWT at:
/// https://dash.cloudflare.com/profile/api-tokens
enum Opt {
    /// Download the page tracker KV data into a CSV file
    Download {
        #[structopt(long, env = "PT_JWT", hide_env_values = true)]
        /// Cloudflare API JWT token
        jwt: String,
        #[structopt(long, env = "PT_ACCOUNT_ID", hide_env_values = true)]
        /// Owner account id of the KV
        account_id: String,
        #[structopt(long, env = "PT_KV_ID", hide_env_values = true)]
        /// KV id
        kv_id: String,
        #[structopt(long, conflicts_with = "output_dir")]
        /// File to write the formatted output
        output: Option<PathBuf>,

        #[structopt(long)]
        /// Folder to write the formatted output based on a format
        output_dir: Option<PathBuf>,

        #[structopt(long, default_value = "%FT%TZ.csv")]
        /// With `output-dir`, this specifies the format used based on `Chronos::format`
        output_format: String,
    },
}

type Credential = (String, String, String);

#[derive(Debug, Deserialize)]
struct ListKeysPayload {
    result: Vec<ListKey>,
}

#[derive(Debug, Deserialize)]
struct ListKey {
    name: String,
}

#[derive(Debug, Serialize)]
struct CsvRecord {
    path: String,
    views: usize,
}

async fn list_keys(client: Client, cred: &Credential) -> Result<Vec<String>> {
    let (jwt, account_id, kv_id) = cred;
    let url = format!(
        "https://api.cloudflare.com/client/v4/accounts/{}/storage/kv/namespaces/{}/keys",
        account_id, kv_id
    );

    let resp = client.get(url).bearer_auth(jwt).send().await?;
    let payload = resp.json::<ListKeysPayload>().await?;

    Ok(payload
        .result
        .into_iter()
        .map(|key| key.name)
        .collect::<Vec<_>>())
}

async fn get_key_value(client: Client, cred: &Credential, key: &str) -> Result<usize> {
    let (jwt, account_id, kv_id) = cred;

    let url = format!(
        "https://api.cloudflare.com/client/v4/accounts/{}/storage/kv/namespaces/{}/values/{}",
        account_id,
        kv_id,
        utf8_percent_encode(key, NON_ALPHANUMERIC)
    );

    let resp = client.get(url).bearer_auth(jwt).send().await?;
    let value = resp.json::<usize>().await?;

    Ok(value)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .format(|buf, record| writeln!(buf, "{}: {}", record.level(), record.args()))
        .init();

    match Opt::from_args() {
        Opt::Download {
            jwt,
            account_id,
            kv_id,
            output,
            output_dir,
            output_format,
        } => {
            let client = Client::new();
            let credentials = (jwt, account_id, kv_id);

            info!("Fetching KV keys");

            let keys = list_keys(client.clone(), &credentials).fuse().await?;

            info!("Found {} keys", keys.len());

            let view_futures = FuturesUnordered::new();

            info!("Fetching KV values");

            for key in keys {
                view_futures.push({
                    let client = client.clone();
                    let credentials = credentials.clone();
                    async move {
                        let view: usize = get_key_value(client, &credentials, &key).await?;

                        info!("Fetched {} -> {}", &key, &view);

                        Ok((key, view))
                    }
                });
            }

            let mut data = view_futures.collect::<Vec<Result<_>>>().await;
            data.sort_by_key(|res| {
                if let Ok((path, _)) = res {
                    Some(path.clone())
                } else {
                    None
                }
            });

            info!("Done fetching all value");

            let output_path = if let Some(path) = output {
                path
            } else if let Some(dir) = output_dir {
                let now: DateTime<Utc> = Utc::now();

                dir.join(now.format(&output_format).to_string())
            } else {
                unreachable!()
            };

            let mut wtr = Writer::from_path(&output_path)?;

            info!("Opening and writing data to {}", output_path.display());

            for view_res in data {
                let (path, views) = view_res?;

                wtr.serialize(CsvRecord { path, views })?;
            }

            wtr.flush()?;

            info!("Done writing data");
        }
    }

    Ok(())
}
