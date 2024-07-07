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
            _ => return None,
        };
        if id != ret as i32 {
            unreachable!("mislabeled id")
        }
        Some(ret)
    }

    pub fn mime(self) -> &'static str {
        match self {
            MediaType::Xhtml => "application/xhtml+xml",
            MediaType::Xml => todo!(),
            MediaType::Png => "image/png",
            MediaType::Jpg => "image/jpeg",
            MediaType::Svg => "image/svg+xml",
            MediaType::Css => "text/css",
        }
    }

}


pub struct ObjectCache {
    conn: Connection,
}

impl ObjectCache {
    pub fn new(conn: Connection) -> Result<Self> {
        conn.execute_batch("
CREATE TABLE cache_entries (id INTEGER PRIMARY KEY AUTOINCREMENT,
                            url TEXT KEY,
                            content BLOB);
")?;
        Ok(ObjectCache { conn })
    }

    pub fn set(&self, key: &str, val: &[u8]) -> Result<()> {
        let mut stmt = self.conn.prepare_cached("INSERT INTO cache_entries (url, content) VALUES (?1, ?2)")?;
        stmt.execute((key, val))?;
        Ok(())
    }

    pub fn get<'a>(&'a self, key: &str) -> Result<Option<Blob<'a>>> {
        let mut stmt = self.conn.prepare_cached("SELECT id FROM cache_entries WHERE url=?1 LIMIT 1")?;
        let id: Option<i64> = stmt.query_row([key], |row| {
            row.get(0)
        }).optional()?;
        let Some(id) = id else { return Ok(None) };
        let blob = self.conn.blob_open(rusqlite::DatabaseName::Main, "cache_entries", "content", id, true)?;
        Ok(Some(blob))
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
        cache.set("key1", b"asdfasdf").unwrap();
        let mut blob = cache.get("key1").unwrap().unwrap();
        let mut buf = String::with_capacity(16);
        blob.read_to_string(&mut buf).unwrap();
        assert_eq!(buf, "asdfasdf")
    }
}
