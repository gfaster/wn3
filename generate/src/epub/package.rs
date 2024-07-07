//! for creating the `.opf` package document
//!
//! <https://www.w3.org/TR/epub/#sec-package-doc>
#![allow(dead_code)]

use std::{collections::HashMap, rc::Rc, time::SystemTime};

use crate::util::{OptSetting, Setting};

pub struct OpfBuilder {
    pub language: Setting,
    pub title: Setting,
    pub creator: Setting,
    pub publisher: Setting,
    pub date: SystemTime,

    pub id_unique: Option<IdentifierType>,
    pub id_isbn10: OptSetting,
    pub id_isbn13: OptSetting,
    pub id_url: OptSetting,

    /// every item in reading order
    pub manifest: Vec<ManifestItem>,
}

#[derive(Debug)]
pub struct OpfError {
    // phase 1 errors
    no_nav: bool,
    no_uniq_id: bool,
    duplicate_manifest_item: bool,

    // phase 2 errors
    uniq_id_unset: bool,
}

impl OpfError {
    fn any(&self) -> bool {
        self.no_nav |
        self.no_uniq_id |
        self.uniq_id_unset |
        self.duplicate_manifest_item
    }
}

pub struct OpfSpec {
    language: Setting,
    title: Setting,
    creator: Setting,
    publisher: Setting,
    date: SystemTime,

    id_unique: IdentifierType,
    id_isbn10: OptSetting,
    id_isbn13: OptSetting,
    id_url: OptSetting,

    manifest_nav: ManifestItem,
    manifest_cover: Option<ManifestItem>,
    manifest: HashMap<Rc<str>, ManifestItem>,
    spine: Vec<Rc<str>>,
}

impl OpfBuilder {
    pub const fn new() -> Self {
        Self {
            language: Setting::dft("en"),
            title: Setting::dft("Ebook"),
            creator: Setting::dft("anonymous"),
            publisher: Setting::dft("unknown"),
            date: SystemTime::UNIX_EPOCH,
            id_unique: None,
            id_isbn10: OptSetting::new(),
            id_isbn13: OptSetting::new(),
            id_url: OptSetting::new(),
            manifest: Vec::new(),
        }
    }

    pub fn finish(self) -> Result<OpfSpec, OpfError> {
        let OpfBuilder {
            language,
            title,
            creator,
            publisher,
            date,
            id_unique,
            id_isbn10,
            id_isbn13,
            id_url,
            manifest,
        } = self;
        let mut e = OpfError {
            no_nav: false,
            no_uniq_id: false,
            uniq_id_unset: false,
            duplicate_manifest_item: false,
        };
        if id_unique.is_none() {
            e.uniq_id_unset = true;
        }
        let manifest_len = manifest.len();
        let (mut manifest, mut spine): (HashMap<_, _>, Vec<_>) = manifest.into_iter().map(|m| {
            let id = Rc::from(m.id());
            ((Rc::clone(&id), m), id)
        }).collect();
        spine.retain(|i| manifest[i].media_type == "application/xhtml+xml");
        if !manifest.contains_key("nav") {
            e.no_nav = true;
        }
        if manifest.len() != manifest_len {
            e.duplicate_manifest_item = true;
        }
        if e.any() {
            return Err(e);
        }
        let id_unique = id_unique.unwrap();
        {
            let is_set = match id_unique {
                IdentifierType::Url => id_url.is_set(),
                IdentifierType::Isbn10 => id_isbn10.is_set(),
                IdentifierType::Isbn13 => id_isbn13.is_set(),
            };
            if !is_set {
                e.uniq_id_unset = true;
            }
        }
        let manifest_nav = manifest.remove("nav").unwrap();
        let manifest_cover = manifest.remove("cover");
        let date = if date == SystemTime::UNIX_EPOCH {
            SystemTime::now()
        } else {
            date
        };

        if e.any() {
            return Err(e)
        }

        Ok(OpfSpec {
            language,
            title,
            creator,
            publisher,
            date,
            id_unique,
            id_isbn10,
            id_isbn13,
            id_url,
            manifest_nav,
            manifest_cover,
            manifest,
            spine,
        })
    }
}

impl Default for OpfBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl OpfSpec {
    pub fn write(&self, w: &mut impl std::io::Write) -> std::io::Result<()> {
        use super::xml::*;
        let mut w = XmlSink::new(w)?;
        let unique = match self.id_unique {
            IdentifierType::Url => "url",
            IdentifierType::Isbn10 => "isbn10",
            IdentifierType::Isbn13 => "isbn13",
        };
        let mut pkg = w.mkel("package", [
            ("version", "3.0"),
            ("xml:lang", "en"),
            ("xmlns", "http://www.idpf.org/2007/opf"),
            ("unique-identifier", unique)
        ])?;
        {
            let mut metadata = pkg.mkel("metadata", [("xmlns:dc", "http://purl.org/dc/elements/1.1/")])?;
            if let Some(url) = self.id_url.get() {
                metadata.mkel("dc:identifier", [("id", "url")])?.write_field(url)?;
            }
            if let Some(isbn) = self.id_isbn10.get() {
                metadata.mkel("dc:identifier", [("id", "isbn10")])?.write_field(isbn)?;
            }
            if let Some(isbn) = self.id_isbn13.get() {
                metadata.mkel("dc:identifier", [("id", "isbn13")])?.write_field(isbn)?;
            }
            // TODO: make this not fake
            metadata.mkel("dc:date", [])?.write_field("2024-07-07")?;
        }
        {
            let mut manifest = pkg.mkel("manifest", [])?;
            manifest.mkel_selfclosed("item", [
                ("id", "nav"),
                ("href", &*self.manifest_nav.href),
                ("media-type", self.manifest_nav.media_type),
                ("properties", "nav"),
            ])?;
            if let Some(cover) = &self.manifest_cover {
                manifest.mkel_selfclosed("item", [
                    ("id", cover.id()),
                    ("href", &*cover.href),
                    ("media-type", cover.media_type),
                    ("properties", "cover-image"),
                ])?;
            }
            for (id, item) in &self.manifest {
                manifest.mkel_selfclosed("item", [
                    ("id", &**id),
                    ("href", &*item.href),
                    ("media-type", item.media_type)
                ])?;
            }
        }
        {
            let mut spine = pkg.mkel("spline", [])?;
            for id in &self.spine {
                spine.mkel_selfclosed("itemref", [("idref", &**id)])?;
            }
        }
        Ok(())
    }
}

/// an `<item />` in manifest
///
/// the `id` field is derived from the `href` file stem, `media-type` is inferred from file
/// extension by default.
///
/// the following `id`s are reserved:
/// - "cover" (`cover-image`)
/// - "nav"
pub struct ManifestItem {
    href: Box<str>,
    media_type: &'static str
}

impl ManifestItem {
    pub fn id(&self) -> &str {
        let (stem, _ext) = self.href.rsplit_once('.').expect("href has stem");
        stem.rsplit_once('/').map_or(stem, |(_, id)| id)
    }

    pub fn try_new(href: &str) -> Option<Self> {
        let (stem, ext) = href.rsplit_once('.')?;
        let media_type = match ext {
            "xhtml" => "application/xhtml+xml",
            "png" => "image/png",
            "jpeg" | "jpg" => "image/jpeg",
            "svg" => "image/svg+xml",
            "css" => "text/css",
            _ => return None
        };
        let id = stem.rsplit_once('/').map_or(stem, |(_, id)| id);
        if id.is_empty() {
            return None
        }
        Some(Self {
            href: href.into(),
            media_type,
        })
    }
}

pub enum IdentifierType {
    Url,
    Isbn10,
    Isbn13,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let builder = OpfBuilder {
            title: "test".into(),
            id_isbn13: "978-1-56619-909-4".into(),
            id_unique: Some(IdentifierType::Isbn13),
            manifest: vec![
                ManifestItem::try_new("nav.xhtml").unwrap(),
                ManifestItem::try_new("chapter-1.xhtml").unwrap(),
            ],
            ..Default::default()
        };
        builder.finish().unwrap();
    }
}
