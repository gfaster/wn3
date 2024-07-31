//! for creating the `.opf` package document
//!
//! <https://www.w3.org/TR/epub/#sec-package-doc>

use std::{rc::Rc, time::SystemTime};
use ahash::HashMap;

use fetch::MediaType;
use time::{format_description, OffsetDateTime};

use crate::util::{OptSetting, Setting};

pub struct OpfBuilder {
    pub language: Setting,
    pub title: Setting,
    pub subtitle: OptSetting,
    pub publisher: OptSetting,
    pub date: SystemTime,

    /// list of identifiers according to <http://purl.org/dc/terms/identifier>
    ///
    /// There must be at least one identifier. The first element in this `Vec` is chosen as the
    /// unique identifier. Use `sort_ids` to sort based on [`IdentifierType`] `Ord` impl which is
    /// the suggested ordering
    pub identifiers: Vec<(IdentifierType, Box<str>)>,

    /// contributors (creators) with their MARC relator role
    pub contributors: Vec<(ContributorRole, Box<str>)>,

    /// every item in reading order.
    ///
    /// The spine is built from this by stripping out every non-xhtml manifest item
    pub manifest: Vec<ManifestItem>,
}

#[derive(Debug)]
pub struct OpfError {
    // phase 1 errors
    no_nav: bool,
    no_identifiers: bool,
    duplicate_manifest_item: bool,
    multiple_nav: bool,
    multiple_cover: bool,
    conflicting_manifest_properties: bool,

    // phase 2 errors
}

impl OpfError {
    fn any(&self) -> bool {
        self.no_nav |
        self.no_identifiers |
        self.duplicate_manifest_item |
        self.multiple_nav | 
        self.multiple_cover |
        self.conflicting_manifest_properties
    }
}

pub struct OpfSpec {
    language: Setting,
    title: Setting,
    subtitle: OptSetting,
    publisher: OptSetting,
    date: SystemTime,

    identifiers: Vec<(IdentifierType, Box<str>)>,
    contributors: Vec<(ContributorRole, Box<str>)>,

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
            subtitle: OptSetting::new(),
            publisher: OptSetting::new(),
            date: SystemTime::UNIX_EPOCH,
            manifest: Vec::new(),
            identifiers: Vec::new(),
            contributors: Vec::new(),
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
            subtitle,
            publisher,
            date,
            manifest,
            identifiers,
            contributors,
        } = self;
        let mut e = OpfError {
            no_nav: false,
            duplicate_manifest_item: false,
            no_identifiers: false,
            multiple_nav: false,
            multiple_cover: false,
            conflicting_manifest_properties: false,
        };
        if identifiers.is_empty() {
            e.no_identifiers = true;
        }
        let manifest_len = manifest.len();
        let mut found_nav = false;
        let mut found_cover = false;
        for item in &manifest {
            if item.props.contains(ManifestProperties::NAV) {
                if !found_nav {
                    found_nav = true
                } else {
                    e.multiple_nav = true;
                }
            }
            if item.props.contains(ManifestProperties::COVER_IMAGE) {
                if !found_cover {
                    found_cover = true
                } else {
                    e.multiple_cover = true;
                }
            }
            if item.props.contains(ManifestProperties::COVER_IMAGE) && !item.media_type.is_image() {
                e.conflicting_manifest_properties = true
            }
            if item.props.contains(ManifestProperties::NAV) && item.media_type != MediaType::Xhtml {
                e.conflicting_manifest_properties = true
            }
            // TODO: check more illegal variations
        }
        if !found_nav {
            e.no_nav = true;
        }
        if e.any() {
            return Err(e);
        }
        let mut manifest_nav = None;
        let mut manifest_cover = None;
        let (mut manifest, mut spine): (HashMap<_, _>, Vec<_>) = manifest.into_iter().map(|m| {
            let id = Rc::from(m.id());
            if m.props.contains(ManifestProperties::NAV) {
                manifest_nav = Some(Rc::clone(&id));
            }
            if m.props.contains(ManifestProperties::COVER_IMAGE) {
                manifest_cover = Some(Rc::clone(&id));
            }
            ((Rc::clone(&id), m), id)
        }).collect();
        spine.retain(|i| manifest[i].media_type == MediaType::Xhtml);
        if manifest.len() != manifest_len {
            e.duplicate_manifest_item = true;
        }
        if e.any() {
            return Err(e);
        }
        let manifest_nav = manifest.remove(&manifest_nav.unwrap()).unwrap();
        let manifest_cover = manifest_cover.map(|id| manifest.remove(&id).unwrap());
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
            subtitle,
            publisher,
            contributors,
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

            metadata.mkel("dc:title", [("id", "title_main")])?.write_field(self.title())?;
            metadata.mkel("meta", [("refines", "#title_main"), ("property", "title-type")])?.write_field("main")?;
            if let Some(subtitle) = self.subtitle.get() {
                metadata.mkel("dc:title", [("id", "title_sub")])?.write_field(subtitle)?;
                metadata.mkel("meta", [("refines", "#title_sub"), ("property", "title-type")])?.write_field("subtitle")?;
            }

            for (i, &(_ty, ref id)) in self.identifiers.iter().enumerate() {
                metadata.mkel("dc:identifier", [("id", &*format!("identifier_{i}"))])?.write_field(id)?;
                metadata.write_lf()?;
            }
            metadata.mkel("dc:date", [])?.write_field(datetime.date())?;
            let datestr = datetime.format(
                &format_description::parse("[year]-[month]-[day]T[hour]:[minute]:[second]Z")
                    .expect("valid format description")).unwrap();
            metadata.write_lf()?;
            metadata.mkel("meta", [("property","dcterms:modified")])?.write_field(datestr)?;
            metadata.write_lf()?;
            for (i, &(role, ref creator)) in self.contributors.iter().enumerate() {
                let id =    format!("creator{i:03}");
                let selid = format!("#{id}");
                // TODO: allow specification of attribution level
                let attribution = if role == ContributorRole::Author {
                    "dc:creator"
                } else {
                    "dc:contributor"
                };
                metadata.mkel(attribution, [("id", &*id)])?.write_field(&**creator)?;
                metadata.write_lf()?;
                metadata.mkel("meta", [
                    ("refines", &*selid),
                    ("property", "role"),
                    ("scheme", "marc:relators"),
                ])?.write_field(role.marc_code())?;
                metadata.write_lf()?;
            }
            metadata.mkel("dc:language", [])?.write_field(&*self.language)?;
            if let Some(publisher) = self.publisher.get() {
                metadata.mkel("dc:publisher", [])?.write_field(publisher)?;
                metadata.write_lf()?;
            }
        }
        {
            let mut manifest = pkg.mkel("manifest", [])?;
            {
                let nav_prop = self.manifest_nav.props.attribute_val().unwrap();
                assert!(nav_prop.contains("nav"));
                manifest.mkel_selfclosed("item", [
                    ("id", "nav"),
                    ("href", &*self.manifest_nav.href),
                    ("media-type", self.manifest_nav.media_type.mime()),
                    ("properties", &nav_prop),
                ])?;
                manifest.write_lf()?;
            }
            if let Some(cover) = &self.manifest_cover {
                manifest.mkel_selfclosed("item", [
                    ("id", cover.id()),
                    ("href", &*cover.href),
                    ("media-type", cover.media_type.mime()),
                    ("properties", &cover.props.attribute_val().unwrap()),
                ])?;
                manifest.write_lf()?;
            }
            for (id, item) in &self.manifest {
                let attr = item.props.attribute_val();
                manifest.mkel_selfclosed("item", [
                    ("id", &**id),
                    ("href", &*item.href),
                    ("media-type", item.media_type.mime())
                ].into_iter().chain(attr.as_deref().map(|a| ("properties", a))))?;
                manifest.write_lf()?;
            }
        }
        {
            let mut spine = pkg.mkel("spine", [])?;
            for id in &self.spine {
                spine.mkel_selfclosed("itemref", [("idref", &**id)])?;
                spine.write_lf()?;
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

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ManifestProperties: u32 {
        const MATHML = 1 << 0;
        const REMOTE_RESOURCES = 1 << 1;
        const SCRIPTED = 1 << 2;
        const SVG = 1 << 3;
        const COVER_IMAGE = 1 << 4;
        const NAV = 1 << 5;
    }
}

impl ManifestProperties {
    /// get the attribute field
    ///
    /// ```
    /// type Prop = ManifestItemProperties;
    ///
    /// assert_eq!(Prop::empty().attribute_val(), None);
    /// assert_eq!(Prop::SCRIPTED.attribute_val().unwrap(), "scripted");
    /// assert_eq!(Prop::REMOTE_RESOURCES.attribute_val().unwrap(), "remote-resources");
    ///
    /// let compound = Prop::COVER_IMAGE | Prop::SVG | Prop::REMOTE_RESOURCES;
    /// assert_eq!(compount.attribute_val().unwrap(), "remote-resources svg cover-image");
    /// ```
    pub fn attribute_val(self) -> Option<String> {
        let mut it = self.iter_names();
        let mut buf = String::from(it.next()?.0);
        for (name, _) in it {
            buf.push(' ');
            buf.push_str(name);
        }
        buf.make_ascii_lowercase();
        // this is slow, but the other real option is use unsafe. I don't think it'll matter at
        // all.
        let buf = buf.chars().map(|c| if c == '_' { '-' } else { c }).collect();

        Some(buf)
    }
}

/// an `<item />` in manifest
///
/// the `id` field is derived from the `href` file stem, `media-type` is inferred from file
/// extension by default.
pub struct ManifestItem {
    href: Box<str>,
    media_type: MediaType,
    pub props: ManifestProperties,
}

impl ManifestItem {
    /// gets the id of this element, equivalent to the basename of the path
    ///
    /// ```
    /// assert_eq!(ManifestItem::new("assets/cover.png").id(), "cover");
    /// assert_eq!(ManifestItem::new("book.xhtml").id(), "book");
    /// assert_eq!(ManifestItem::new("chapter_1.xhtml").id(), "chapter_1");
    /// assert_eq!(ManifestItem::new("part1/images/im1.png").id(), "im1");
    /// ```
    pub fn id(&self) -> &str {
        let (stem, _ext) = self.href.rsplit_once('.').expect("href has stem");
        stem.rsplit_once('/').map_or(stem, |(_, id)| id)
    }

    pub fn new(href: impl Into<Box<str>>) -> Self {
        Self::try_new(href).expect("invalid href")
    }

    /// don't infer media type or properties
    pub fn new_explicit(href: impl Into<Box<str>>, ty: MediaType) -> Self {
        Self {
            href: href.into(),
            media_type: ty,
            props: ManifestProperties::empty(),
        }
    }

    /// create a new item from path. 
    ///
    /// A few ids (basename) will implicitly enable properties:
    /// - `nav` implies [`ManifestProperties::NAV`]
    /// - `cover` implies [`ManifestProperties::COVER_IMAGE`]
    pub fn try_new(href: impl Into<Box<str>>) -> Option<Self> {
        let href = href.into();
        let (stem, ext) = href.rsplit_once('.')?;
        let media_type = match ext {
            "xhtml" => MediaType::Xhtml,
            "png" => MediaType::Png,
            "jpeg" | "jpg" => MediaType::Jpg,
            "svg" => MediaType::Svg,
            "css" => MediaType::Css,
            _ => return None
        };
        let id = stem.rsplit_once('/').map_or(stem, |(_, id)| id);
        if id.is_empty() {
            return None
        }
        let mut props = ManifestProperties::empty();
        if id == "nav" {
            props |= ManifestProperties::NAV;
        } else if id == "cover" {
            props |= ManifestProperties::COVER_IMAGE;
        }
        Some(Self {
            href: href.into(),
            media_type,
            props,
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
