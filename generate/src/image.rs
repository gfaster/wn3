use std::{fmt::Display, sync::Arc};

use ahash::RandomState;

use crate::html_writer::{EscapeAttr, EscapeMd};


#[cfg(debug_assertions)]
fn assert_no_collisions(url: &str, hash: u64) {
    use std::sync::Mutex;
    use ahash::{HashMap, HashMapExt};
    static MAP: Mutex<Option<HashMap<u64, Box<str>>>> = Mutex::new(None);
    let mut lock = MAP.lock().unwrap();
    if lock.is_none() {
        *lock = Some(HashMap::new());
    }
    let m = lock.as_mut().unwrap();
    m.entry(hash).and_modify(|x| assert_eq!(&**x, url, "hash collision")).or_insert_with(|| url.into());
}

#[cfg(not(debug_assertions))]
fn assert_no_collisions(_url: &str, _hash: u64) {}

pub fn url_id(url: &str) -> u64 {
    // cat /dev/urandom | tr -cd 'a-f0-9' | head -c 8
    let ret = RandomState::with_seeds(0x48, 0xb0, 0x7e, 0x03).hash_one(url);
    assert_no_collisions(url, ret);
    ret
}

#[derive(Debug)]
pub struct Image {
    /// this will need to be acquired from the Store in such a way that we don't duplicate images
    id: u64,
    url: Arc<str>,
    pub alt: Option<String>,
}

impl Image {
    pub fn new(url: impl Into<Arc<str>>) -> Self {
        let url = url.into();
        Image { id: url_id(&url), url, alt: None }
    }
    
    pub fn url(&self) -> &Arc<str> {
        &self.url
    }

    fn src(&self) -> impl Display {
        struct D(u64);
        impl Display for D {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "assets/image_{id:016x}", id = self.0)
            }
        }
        D(self.id)
    }
}

impl Display for Image {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let alt = self.alt.as_deref().unwrap_or("an image without alt text");
        let src = self.src();
        if f.alternate() {
            let alt = EscapeMd(alt);
            write!(f, "![{alt}]({src})")
        } else {
            let alt = EscapeAttr(alt);
            write!(f, r#"<img src="{src}" alt="{alt}" />"#)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn src_fmt() {
        let img = Image {
            id: 0x00,
            ..Image::new("https://example.com")
        };
        println!("{}", img.src());
        panic!()
    }

}
