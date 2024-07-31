use std::{fmt::Display, sync::Arc};

use ahash::RandomState;
use bytes::Bytes;
use fetch::MediaType;

use crate::{epub::package::ManifestItem, html_writer::{EscapeAttr, EscapeMd}};


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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageId(u64);
impl Display for ImageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "image_{id:016x}", id = self.0)
    }
}

#[derive(Debug)]
pub struct Image {
    url: Arc<str>,
    pub alt: Option<String>,
}

impl Image {
    pub fn new(url: impl Into<Arc<str>>) -> Self {
        Image {
            url: url.into(),
            alt: None,
        }
    }

    pub fn id(&self) -> ImageId {
        ImageId(url_id(&self.url))
    }

    pub(crate) fn resolve_with(self, ty: MediaType, data: Bytes) -> ResolvedImage {
        ResolvedImage { 
            id: self.id(),
            alt: self.alt,
            media_type: ty,
            data
        }
    }

    pub fn url(&self) -> &Arc<str> {
        &self.url
    }

}

#[derive(Debug)]
pub struct ResolvedImage {
    id: ImageId,
    pub alt: Option<String>,
    pub(crate) media_type: MediaType,
    pub(crate) data: Bytes,
}

impl Display for ResolvedImage {
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

impl ResolvedImage {
    pub fn manifest_item(&self) -> ManifestItem {
        ManifestItem::new_explicit(self.src().to_string(), self.media_type)
    }

    pub(crate) fn id(&self) -> ImageId {
        self.id
    }

    pub(crate) fn src_with_basename<'a>(&self, base: impl Display + 'a) -> impl Display + 'a {
        struct D<B>(B, MediaType);
        impl<B: Display> Display for D<B> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "assets/{base}.{ext}", base = self.0, ext = self.1.extension())
            }
        }
        let ty = self.media_type;
        D(base, ty)
    }

    pub(crate) fn src(&self) -> impl Display {
        self.src_with_basename(self.id)
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//
//     #[test]
//     fn src_fmt() {
//         let img = Image {
//             ..Image::new("https://example.com")
//         };
//         println!("{}", img.src());
//         panic!()
//     }
//
// }
