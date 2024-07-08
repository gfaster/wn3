use std::io::prelude::Read;

use bytes::Bytes;
use rusqlite::{blob::Blob, Connection, OptionalExtension, Result};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    Xhtml = 0,
    Xml = 1,
    Png = 2,
    Jpg = 3,
    Svg = 4,
    Css = 5,
    Gif = 6,
    Webp = 7,
    Html = 8,
}

impl MediaType {

    pub fn new(id: i32) -> Self {
        Self::try_new(id).expect("valid id")
    }

    pub fn try_new(id: i32) -> Option<Self> {
        let ret = match id {
            0 => Self::Xhtml,
            1 => Self::Xml,
            2 => Self::Png,
            3 => Self::Jpg,
            4 => Self::Svg,
            5 => Self::Css,
            6 => Self::Gif,
            7 => Self::Webp,
            8 => Self::Html,
            _ => return None,
        };
        if id != ret as i32 {
            unreachable!("mislabeled id")
        }
        Some(ret)
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "application/xhtml+xml" => MediaType::Xhtml,
            "application/xml" => MediaType::Xml,
            "image/png" => MediaType::Png,
            "image/jpeg" => MediaType::Jpg,
            "image/svg+xml" => MediaType::Svg,
            "text/css" => MediaType::Css,
            "image/gif" => MediaType::Gif,
            "image/webp" => MediaType::Webp,
            "text/html" => MediaType::Html,
            _ => panic!("unknown type {s:?}")
        }
    }

    pub fn mime(self) -> &'static str {
        match self {
            MediaType::Xhtml => "application/xhtml+xml",
            MediaType::Xml => "application/xml",
            MediaType::Png => "image/png",
            MediaType::Jpg => "image/jpeg",
            MediaType::Svg => "image/svg+xml",
            MediaType::Css => "text/css",
            MediaType::Gif => "image/gif",
            MediaType::Webp => "image/webp",
            MediaType::Html => "text/html",
        }
    }

}


pub struct ObjectCache {
    conn: Connection,
}

impl ObjectCache {
    pub fn new(conn: Connection) -> Result<Self> {
        conn.execute_batch("
CREATE TABLE IF NOT EXISTS cache_entries (id INTEGER PRIMARY KEY AUTOINCREMENT,
                            url TEXT KEY,
                            type INTEGER,
                            content BLOB);
")?;
        Ok(ObjectCache { conn })
    }

    pub fn set(&self, key: &str, val: &[u8], ty: MediaType) -> Result<()> {
        let mut stmt = self.conn.prepare_cached("INSERT INTO cache_entries (url, type, content) VALUES (?1, ?2, ?3)")?;
        stmt.execute((key, ty as i64, val))?;
        Ok(())
    }

    pub fn get<'a>(&'a self, key: &str) -> Result<Option<(MediaType, Blob<'a>)>> {
        let mut stmt = self.conn.prepare_cached("SELECT id, type FROM cache_entries WHERE url=?1 LIMIT 1")?;
        let id: Option<(i64, i64)> = stmt.query_row([key], |row| {
            Ok((row.get(0)?, row.get(1)?))
        }).optional()?;
        let Some((id, ty)) = id else { return Ok(None) };
        let ty = MediaType::new(ty as i32);
        let blob = self.conn.blob_open(rusqlite::DatabaseName::Main, "cache_entries", "content", id, true)?;
        Ok(Some((ty, blob)))
    }

    #[allow(dead_code)]
    pub fn get_string(&self, key: &str) -> Result<Option<(MediaType, String)>> {
        let Some((ty, mut blob)) = self.get(key)? else {
            return Ok(None)
        };
        let mut buf = String::with_capacity(blob.len());
        blob.read_to_string(&mut buf).unwrap();
        Ok(Some((ty, buf)))
    }

    #[allow(dead_code)]
    pub fn get_bytes(&self, key: &str) -> Result<Option<(MediaType, Bytes)>> {
        let Some((ty, mut blob)) = self.get(key)? else {
            return Ok(None)
        };
        let mut buf = vec![0; blob.len()];
        blob.read_exact(&mut buf).unwrap();
        Ok(Some((ty, buf.into())))
    }
}

#[cfg(test)]
mod tests {
    use std::io::prelude::*;
    use super::*;

    fn new_cache() -> ObjectCache {
        ObjectCache::new(rusqlite::Connection::open_in_memory().unwrap()).unwrap()
    }

    #[test]
    fn it_works() {
        let cache = new_cache();
        cache.set("key1", b"asdfasdf", MediaType::Jpg).unwrap();
        let mut blob = cache.get("key1").unwrap().unwrap();
        let mut buf = String::with_capacity(16);
        blob.1.read_to_string(&mut buf).unwrap();
        assert_eq!(buf, "asdfasdf")
    }

    #[test]
    fn get_and_set() {
        let cache = new_cache();
        assert!(cache.get("key1").unwrap().is_none());
        cache.set("key1", b"asdfasdf", MediaType::Jpg).unwrap();
        let res = cache.get_string("key1").unwrap().unwrap();
        assert_eq!(res.1, "asdfasdf");
    }

    #[test]
    fn use_url() {
        let cache = new_cache();
        assert!(cache.get("https://example.com").unwrap().is_none());
        cache.set("https://example.com", b"asdfasdf", MediaType::Html).unwrap();
        let res = cache.get_string("https://example.com").unwrap().unwrap();
        assert_eq!(res.1, "asdfasdf");
    }
}
