use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use bytes::Bytes;
use log::{info, trace};
use ratelimit::wait_your_turn;

mod cache;
pub use cache::MediaType;
use cache::ObjectCache;
use url::Url;

mod ratelimit;

#[derive(Clone)]
pub struct FetchContext {
    cache: Arc<Mutex<cache::ObjectCache>>,
    client: ureq::Agent,
    pub offline: bool,
}

impl FetchContext {
    pub fn new_cfg(
        conn: rusqlite::Connection,
        client: ureq::Agent,
        offline: bool,
    ) -> rusqlite::Result<Self> {
        Ok(FetchContext {
            cache: Arc::new(Mutex::new(ObjectCache::new(conn)?)),
            client,
            offline,
        })
    }

    pub fn new(conn: rusqlite::Connection, client: ureq::Agent) -> rusqlite::Result<Self> {
        Ok(FetchContext {
            cache: Arc::new(Mutex::new(ObjectCache::new(conn)?)),
            client,
            offline: false,
        })
    }

    /// gets url from store, but will not touch network
    pub fn fetch_local(&self, url: &str) -> Result<(MediaType, Bytes)> {
        if let Some(bytes) = self
            .cache
            .lock()
            .unwrap()
            .get_bytes(url)
            .context("db access failed")?
        {
            trace!("{url} found in cache");
            return Ok(bytes);
        }
        bail!("{url} not found in cache")
    }

    pub fn fetch(&self, url: &Url) -> Result<(MediaType, Bytes)> {
        if url.scheme() == "file" {
            let path = url
                .to_file_path()
                .map_err(|_| anyhow!("invalid file path"))?;
            let ext = path
                .extension()
                .context("no extension")?
                .to_str()
                .context("path not utf-8")?;
            let ty = MediaType::from_extension(ext)
                .with_context(|| format!("extension {ext} is invalid"))?;
            let data: Bytes = std::fs::read(&path)
                .with_context(|| format!("could not read {}", path.display()))?
                .into();
            return Ok((ty, data));
        }
        if let Some(bytes) = self.cache.lock().unwrap().get_bytes(url.as_str()).unwrap() {
            trace!("{url} found in cache");
            return Ok(bytes);
        }
        if self.offline {
            bail!("cannot fetch {url} because offline is enabled")
        }
        let domain = url.domain().unwrap();
        trace!("getting in line to access {}", domain);

        // we use 65 seconds to avoid connection closed before message completed errors
        wait_your_turn(domain, Duration::from_secs(65));
        info!("fetching url {url}");

        let res = self.client.request_url("GET", url).call();
        let resp = match res {
            Ok(succ) => succ,
            Err(e) => {
                return Err(e.into());
            }
        };
        let Some(resp_ty) = resp.header("Content-Type") else {
            bail!("TODO: handle no content-type")
        };
        let resp_ty = MediaType::from_mime(resp_ty.split_once(';').map_or(resp_ty, |(x, _rem)| x));
        let bytes: Bytes = {
            use std::io::prelude::*;

            let max = 1024 * 1024 * 1024;
            let mut reader = resp.into_reader().take(max);
            let mut buf = Vec::with_capacity(1024 * 16);
            reader.read_to_end(&mut buf).context("failed to get body")?;
            buf.into()
        };
        self.cache
            .lock()
            .unwrap()
            .set(url.as_str(), &bytes, resp_ty)?;

        trace!("completed request to {domain:?}");

        Ok((resp_ty, bytes))
    }

    pub fn manual_set_cache(&self, url: &Url, contents: &[u8], ty: MediaType) -> Result<()> {
        self.cache.lock().unwrap().set(url.as_str(), contents, ty)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_blocks_req() {
        let a = FetchContext::new_cfg(
            rusqlite::Connection::open_in_memory().unwrap(),
            ureq::agent(),
            true,
        )
        .unwrap();
        let err = a.fetch(&"http://localhost".parse().unwrap()).unwrap_err();
        assert!(err.to_string().contains("offline is enabled"));
    }

    #[test]
    fn offline_allows_cached() {
        let a = FetchContext::new_cfg(
            rusqlite::Connection::open_in_memory().unwrap(),
            ureq::agent(),
            true,
        )
        .unwrap();
        let url: Url = "https://localhost".parse().unwrap();
        let contents = "<!DOCTYPE html> <html> <head> </head> <body> </body> </html>";
        a.manual_set_cache(&url, contents.as_bytes(), MediaType::Html)
            .unwrap();
        let (ty, res) = a.fetch(&url).unwrap();
        assert_eq!(ty, MediaType::Html);
        assert_eq!(res, contents.as_bytes());
    }
}
