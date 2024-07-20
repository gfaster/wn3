use std::time::Duration;

use bytes::Bytes;
use ratelimit::wait_your_turn;
use anyhow::Result;

mod cache;
use cache::ObjectCache;
pub use cache::MediaType;

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

    pub async fn fetch(&self, url: &str) -> Result<(MediaType, Bytes)> {
        let url = url.trim();
        if let Some(bytes) = self.cache.get_bytes(url).unwrap() {
            // eprintln!("{url:?} found in cache");
            return Ok(bytes)
        }
        let no_protocol = url
            .trim_start_matches("http")
            .trim_start_matches("s")
            .trim_start_matches("://");
        let domain = no_protocol.split_once('/').map_or(no_protocol, |(x, _rem)| x);
        eprint!("getting in line to access {domain:?}\r");

        wait_your_turn(domain, Duration::from_secs(60)).await;
        eprintln!("fetching url {url}");

        // check cache again in case this was stored earlier
        if let Some(bytes) = self.cache.get_bytes(url).unwrap() {
            eprintln!("{url:?} found in cache after waiting");
            return Ok(bytes)
        }

        let res = self.client.get(url).send().await;
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
        self.cache.set(url, &*bytes, resp_ty)?;

        eprintln!("completed request to {domain:?}");

        Ok((resp_ty, bytes))
    }
}

