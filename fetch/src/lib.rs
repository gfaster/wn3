use std::time::Duration;

use bytes::Bytes;
use ratelimit::wait_your_turn;
use anyhow::{bail, Result};

mod cache;
use cache::ObjectCache;
pub use cache::MediaType;
use reqwest::Url;

mod ratelimit;

pub struct FetchContext {
    cache: cache::ObjectCache,
    client: reqwest::Client,
}

impl FetchContext {
    pub fn new(conn: rusqlite::Connection, client: reqwest::Client) -> rusqlite::Result<Self> {
        Ok(FetchContext { 
            cache: ObjectCache::new(conn)?,
            client
        })
    }

    /// gets url from store, but will not touch network
    pub fn fetch_local(&self, _url: &str) -> Result<(MediaType, Bytes)> {
        todo!()
    }

    pub async fn fetch(&self, url: &Url) -> Result<(MediaType, Bytes)> {
        if let Some(bytes) = self.cache.get_bytes(url.as_str()).unwrap() {
            // eprintln!("{url:?} found in cache");
            return Ok(bytes)
        }
        if url.scheme() == "file" {
            bail!("TODO: handle file:// for url {url}")
        }
        let domain = url.domain().unwrap();
        eprint!("getting in line to access {}\r", domain);

        wait_your_turn(domain, Duration::from_secs(60)).await;
        eprintln!("fetching url {url}");

        // check cache again in case this was stored earlier
        if let Some(bytes) = self.cache.get_bytes(url.as_str()).unwrap() {
            eprintln!("{url:?} found in cache after waiting");
            return Ok(bytes)
        }

        let res = self.client.get(url.clone()).send().await;
        let resp = match res {
            Ok(succ) => {
                succ
            },
            Err(e) if e.is_builder() => {
                panic!("builder malformed: {e}")
            },
            Err(e) => {
                return Err(e.into());
            },
        };
        resp.error_for_status_ref()?;
        // dbg!(resp.headers());
        let Some(resp_ty) = resp.headers().get("Content-Type") else {
            todo!("handle this error")
        };
        let Ok(resp_ty) = resp_ty.to_str() else {
            todo!("handle this error")
        };
        let resp_ty = MediaType::from_str(resp_ty.split_once(';').map_or(resp_ty, |(x, _rem)| x));
        let bytes = resp.bytes().await?;
        self.cache.set(url.as_str(), &*bytes, resp_ty)?;

        eprintln!("completed request to {domain:?}");

        Ok((resp_ty, bytes))
    }
}

