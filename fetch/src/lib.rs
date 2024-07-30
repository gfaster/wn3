use std::{sync::{Arc, Mutex}, time::Duration};

use bytes::Bytes;
use ratelimit::wait_your_turn;
use anyhow::{bail, Context, Result};

mod cache;
use cache::ObjectCache;
pub use cache::MediaType;
use url::Url;

mod ratelimit;

// TODO: make this not async?
#[derive(Clone)]
pub struct FetchContext {
    cache: Arc<Mutex<cache::ObjectCache>>,
    client: ureq::Agent,
}

impl FetchContext {
    pub fn new(conn: rusqlite::Connection, client: ureq::Agent) -> rusqlite::Result<Self> {
        Ok(FetchContext { 
            cache: Arc::new(Mutex::new(ObjectCache::new(conn)?)),
            client
        })
    }

    /// gets url from store, but will not touch network
    pub fn fetch_local(&self, url: &str) -> Result<(MediaType, Bytes)> {
        if let Some(bytes) = self.cache.lock().unwrap().get_bytes(url).context("db access failed")? {
            // eprintln!("{url:?} found in cache");
            return Ok(bytes)
        }
        bail!("{url} not found in cache")
    }

    pub fn fetch(&self, url: &Url) -> Result<(MediaType, Bytes)> {
        if let Some(bytes) = self.cache.lock().unwrap().get_bytes(url.as_str()).unwrap() {
            // eprintln!("{url:?} found in cache");
            return Ok(bytes)
        }
        if url.scheme() == "file" {
            bail!("TODO: handle file:// for url {url}")
        }
        let domain = url.domain().unwrap();
        eprint!("getting in line to access {}\r", domain);

        // we use 65 seconds we tend to get connection closed before message completed errors
        wait_your_turn(domain, Duration::from_secs(65));
        eprintln!("fetching url {url}");

        // check cache again in case this was stored earlier
        // if let Some(bytes) = self.cache.lock().unwrap().get_bytes(url.as_str()).unwrap() {
        //     eprintln!("{url:?} found in cache after waiting");
        //     return Ok(bytes)
        // }

        let res = self.client.request_url("GET", &url).call();
        let resp = match res {
            Ok(succ) => {
                succ
            },
            Err(e) => {
                return Err(e.into());
            },
        };
        // dbg!(resp.headers());
        let Some(resp_ty) = resp.header("Content-Type") else {
            todo!("handle this error")
        };
        let resp_ty = MediaType::from_str(resp_ty.split_once(';').map_or(resp_ty, |(x, _rem)| x));
        let bytes: Bytes = {
            use std::io::prelude::*;

            let max = 1024 * 1024 * 1024;
            let mut reader = resp.into_reader().take(max);
            let mut buf = Vec::with_capacity(1024 * 16);
            reader.read_to_end(&mut buf).context("failed to get body")?;
            buf.into()
        };
        self.cache.lock().unwrap().set(url.as_str(), &*bytes, resp_ty)?;

        eprint!("completed request to {domain:?}                       \r");

        Ok((resp_ty, bytes))
    }
}

