//! for creating the `.opf` package document
//!
//! <https://www.w3.org/TR/epub/#sec-package-doc>

use std::{collections::HashMap, rc::Rc, time::SystemTime};

use time::{format_description, OffsetDateTime};

use crate::util::{OptSetting, Setting};

pub struct OpfBuilder {
    pub language: Setting,
    pub title: Setting,
    pub publisher: OptSetting,
    pub date: SystemTime,

    /// list of identifiers according to <http://purl.org/dc/terms/identifier>
    ///
    /// There must be at least one identifier. The first element in this `Vec` is chosen as the
    /// unique identifier. Use `sort_ids` to sort based on [`IdentifierType`] `Ord` impl which is
    /// the suggested ordering
    pub identifiers: Vec<(IdentifierType, Box<str>)>,

    /// contributers (creators) with their MARC relator role
    pub contributers: Vec<(ContributorRole, Box<str>)>,

    /// every item in reading order
    pub manifest: Vec<ManifestItem>,
}

#[derive(Debug)]
pub struct OpfError {
    // phase 1 errors
    no_nav: bool,
    no_identifiers: bool,
    duplicate_manifest_item: bool,

    // phase 2 errors
}

impl OpfError {
    fn any(&self) -> bool {
        self.no_nav |
        self.no_identifiers |
        self.duplicate_manifest_item
    }
}

pub struct OpfSpec {
    language: Setting,
    title: Setting,
    publisher: OptSetting,
    date: SystemTime,

    identifiers: Vec<(IdentifierType, Box<str>)>,
    contributers: Vec<(ContributorRole, Box<str>)>,

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
            publisher: OptSetting::new(),
            date: SystemTime::UNIX_EPOCH,
            manifest: Vec::new(),
            identifiers: Vec::new(),
            contributers: Vec::new(),
        }
    }

    pub fn add_identifier(&mut self, ty: IdentifierType, val: impl Into<Box<str>>) -> &mut Self {
        self.identifiers.push((ty, val.into()));
        self
    }

    pub fn finish(self) -> Result<OpfSpec, OpfError> {
        let OpfBuilder {
            language,
            title,
            publisher,
            date,
            manifest,
            identifiers,
            contributers,
        } = self;
        let mut e = OpfError {
            no_nav: false,
            duplicate_manifest_item: false,
            no_identifiers: false,
        };
        if identifiers.is_empty() {
            e.no_identifiers = true;
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
            publisher,
            contributers,
            date,
            identifiers,
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
        let datetime: OffsetDateTime = self.date.into();
        let datetime = datetime.to_offset(time::UtcOffset::UTC);
        let mut w = XmlSink::new(w)?;
        let mut pkg = w.mkel("package", [
            ("version", "3.0"),
            ("xml:lang", "en"),
            ("xmlns", "http://www.idpf.org/2007/opf"),
            ("unique-identifier", "identifier_0")
        ])?;
        {
            let mut metadata = pkg.mkel("metadata", [("xmlns:dc", "http://purl.org/dc/elements/1.1/")])?;

            metadata.mkel("dc:title", [])?.write_field(self.title())?;

            for (i, &(_ty, ref id)) in self.identifiers.iter().enumerate() {
                metadata.mkel("dc:identifier", [("id", &*format!("identifier_{i}"))])?.write_field(id)?;
            }
            // TODO: make dc:date not fake
            metadata.mkel("dc:date", [])?.write_field(datetime.date())?;
            let datestr = datetime.format(
                &format_description::parse("[year]-[month]-[day]T[hour]:[minute]:[second]Z")
                    .expect("valid format description")).unwrap();
            metadata.mkel("meta", [("property","dcterms:modified")])?.write_field(datestr)?;
            for (i, &(role, ref creator)) in self.contributers.iter().enumerate() {
                let id =    format!("creator{i:03}");
                let selid = format!("#{id}");
                metadata.mkel("dc:creator", [("id", &*id)])?.write_field(&**creator)?;
                metadata.mkel("meta", [
                    ("refines", &*selid),
                    ("property", "role"),
                    ("scheme", "marc:relators"),
                ])?.write_field(role.marc_code())?;
            }
            metadata.mkel("dc:language", [])?.write_field(&*self.language)?;
            if let Some(publisher) = self.publisher.get() {
                metadata.mkel("dc:publisher", [])?.write_field(publisher)?;
            }
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
            let mut spine = pkg.mkel("spine", [])?;
            for id in &self.spine {
                spine.mkel_selfclosed("itemref", [("idref", &**id)])?;
            }
        }
        drop(pkg);
        w.finish()?;
        Ok(())
    }

    pub fn title(&self) -> &str {
        &*self.title
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

    pub fn new(href: impl Into<Box<str>>) -> Self {
        Self::try_new(href).expect("invalid href")
    }

    pub fn try_new(href: impl Into<Box<str>>) -> Option<Self> {
        let href = href.into();
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IdentifierType {
    Doi,
    Isbn13,
    Isbn10,
    Issn,
    Url,
    Adhoc,
}

/// See: <https://id.loc.gov/vocabulary/relators.html> and
/// <https://idpf.org/epub/20/spec/OPF_2.0.1_draft.htm#Section2.2.6>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContributorRole {
    Author,
    Illustrator,
    Editor,
    Narrator,
    Funder,
    Translator,
    Programmer,
}

impl ContributorRole {
    pub fn marc_code(self) -> &'static str {
        match self {
            ContributorRole::Author => "aut",
            ContributorRole::Illustrator => "ill",
            ContributorRole::Editor => "edt",
            ContributorRole::Narrator => "nrt",
            ContributorRole::Funder => "fnd",
            ContributorRole::Translator => "trl",
            ContributorRole::Programmer => "prg",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let mut builder = OpfBuilder {
            title: "test".into(),
            manifest: vec![
                ManifestItem::try_new("nav.xhtml").unwrap(),
                ManifestItem::try_new("chapter-1.xhtml").unwrap(),
            ],
            ..Default::default()
        };
        builder.add_identifier(IdentifierType::Isbn13, "978-1-56619-909-4");
        builder.finish().unwrap();
    }
}
