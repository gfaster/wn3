use ahash::{HashMap, HashMapExt};
use anyhow::{Context, Result};
use fetch::FetchContext;
use log::error;
use std::{
    collections::hash_map::Entry,
    io::{self, BufWriter, prelude::*},
    rc::Rc,
};
use url::Url;
use zip::{ZipWriter, write::SimpleFileOptions};

use crate::{
    chapter::Chapter,
    epub::{
        self,
        package::{ManifestItem, ManifestProperties},
        xml::XmlSink,
    },
    html_writer::EscapeBody,
    image::{Image, ImageId, ResolvedImage},
    lang::StrLang,
};

use super::package::{ContributorRole, IdentifierType, OpfBuilder};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Compression {
    Store,
    Deflate,
}

pub struct EpubBuilder<'a> {
    opf: OpfBuilder,
    chapters: Vec<Chapter<'a>>,
    /// `(title, first chapter index)`
    sections: Vec<(Box<str>, usize)>,
    cover: Option<Rc<ResolvedImage>>,
    additional_resources: HashMap<ImageId, Rc<ResolvedImage>>,
    compression: Compression,
    chunk_size: usize,
}

impl<'a> EpubBuilder<'a> {
    pub fn new() -> Self {
        Self {
            opf: OpfBuilder::new(),
            chapters: Vec::new(),
            sections: Vec::new(),
            chunk_size: 0,
            additional_resources: HashMap::new(),
            cover: None,
            compression: Compression::Deflate,
        }
    }

    /// set the image, may make a web request if it's not cached
    pub fn set_cover(&mut self, img: Image, cx: &FetchContext) -> Result<&mut Self> {
        let entry = self.additional_resources.entry(img.id());
        match entry {
            Entry::Occupied(o) => {
                self.cover = Some(Rc::clone(o.get()));
            }
            Entry::Vacant(e) => {
                let (ty, bytes) = cx
                    .fetch(
                        &Url::parse(img.url())
                            .with_context(|| format!("{} is invalid url", img.url()))?,
                    )
                    .with_context(|| format!("failed fetching {}", img.url()))?;
                let img = Rc::new(img.resolve_with(ty, bytes));
                self.cover = Some(Rc::clone(&img));
                e.insert(img);
            }
        }
        Ok(self)
    }

    pub fn set_compression(&mut self, compression: Compression) -> &mut Self {
        self.compression = compression;
        self
    }

    /// Whether or not to include table of contents.
    ///
    /// default is `false`
    pub fn include_toc(&mut self, include_toc: bool) -> &mut Self {
        self.opf.include_toc = include_toc;
        self
    }

    /// set the number of chapters that are combined to a single file via total bytes. If
    /// size is 0, then each chapter will get its own file.
    ///
    /// default is 0
    pub fn set_chunk_size(&mut self, size: usize) -> &mut Self {
        self.chunk_size = size;
        self
    }

    pub fn add_section(&mut self, title: impl AsRef<str>) -> &mut Self {
        self.sections
            .push((title.as_ref().into(), self.chapters.len()));
        self
    }

    pub fn add_chapter(&mut self, chapter: Chapter<'a>) -> &mut Self {
        self.additional_resources
            .extend(chapter.rsc.iter().map(|r| (r.id(), Rc::clone(r))));
        self.chapters.push(chapter);
        self
    }

    pub fn extend_chapters(
        &mut self,
        chapters: impl IntoIterator<Item = Chapter<'a>>,
    ) -> &mut Self {
        for ch in chapters {
            self.add_chapter(ch);
        }
        self
    }

    pub fn set_title(&mut self, title: impl Into<StrLang>) -> &mut Self {
        self.opf.title = Some(title.into());
        self
    }

    pub fn add_author(&mut self, author: impl Into<StrLang>) -> &mut Self {
        self.opf
            .contributors
            .push((ContributorRole::Author, author.into()));
        self
    }

    pub fn add_editor(&mut self, editor: impl Into<StrLang>) -> &mut Self {
        self.opf
            .contributors
            .push((ContributorRole::Editor, editor.into()));
        self
    }

    pub fn add_translator(&mut self, translator: impl Into<StrLang>) -> &mut Self {
        self.opf
            .contributors
            .push((ContributorRole::Translator, translator.into()));
        self
    }

    pub fn add_contributor(
        &mut self,
        role: ContributorRole,
        creator: impl Into<StrLang>,
    ) -> &mut Self {
        self.opf.contributors.push((role, creator.into()));
        self
    }

    pub fn set_publisher(&mut self, publisher: impl Into<Box<str>>) -> &mut Self {
        self.opf.publisher.set(publisher.into());
        self
    }

    pub fn add_identifier(
        &mut self,
        ty: IdentifierType,
        identifier: impl Into<Box<str>>,
    ) -> &mut Self {
        self.opf.add_identifier(ty, identifier.into());
        self
    }

    /// sorts identifiers to use the best option for the unique id.
    ///
    /// If more identifiers are added later, this method must be called again for new identifier to
    /// have a chance at being used.
    ///
    /// This uses a stable sort so that duplicate identifier types are kept in the order they were
    /// added in
    pub fn sort_identifiers(&mut self) -> &mut Self {
        self.opf.identifiers.sort_by_key(|i| i.0);
        self
    }

    pub fn finish(mut self, mut w: impl Read + Write + Seek) -> io::Result<()> {
        w.seek(io::SeekFrom::Start(0))?;
        let w = BufWriter::new(w);
        assert!(!self.chapters.is_empty());

        let mut zip = ZipWriter::new(w);
        let stored =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let method = match self.compression {
            Compression::Store => zip::CompressionMethod::Stored,
            Compression::Deflate => zip::CompressionMethod::Deflated,
        };
        let compressed = SimpleFileOptions::default().compression_method(method);
        zip.start_file("mimetype", stored)?;
        zip.write_all(b"application/epub+zip")?;
        zip.add_directory("EPUB", stored)?;
        zip.add_directory("EPUB/css", stored)?;
        zip.add_directory("META-INF", stored)?;
        if !self.additional_resources.is_empty() {
            zip.add_directory("EPUB/assets", stored)?;
        }
        zip.start_file("META-INF/container.xml", stored)?;
        zip.write_all(CONTAINER_XML.as_bytes())?;

        // chunks here are just splitting the chapters into small enough files. I have this being a
        // little silly here because I want to check later if too-small chapters cause problems.
        let chunks: Vec<_> = self
            .chapters
            // .iter()
            // .map(|ch| std::slice::from_ref(ch))
            .chunk_by({
                let mut size = if self.chunk_size > 0 {
                    self.chapters[0].size()
                } else {
                    0
                };
                move |_l, r| {
                    let rsz = r.size();
                    if size >= self.chunk_size {
                        size = rsz;
                        false
                    } else {
                        size += rsz;
                        true
                    }
                }
            })
            .collect();

        self.opf.manifest.push(ManifestItem::new("css/epub.css"));

        self.opf.manifest.push(ManifestItem::new("nav.xhtml"));
        for (i, chunk) in chunks.iter().enumerate() {
            zip.start_file(format!("EPUB/chunk_{i}.xhtml"), compressed)?;
            write_chunk(&mut zip, chunk)?;
            self.opf
                .manifest
                .push(ManifestItem::new(format!("chunk_{i}.xhtml")));
        }

        for (_id, rsc) in self.additional_resources {
            let file = format!("EPUB/{}", rsc.src());
            zip.start_file(file, compressed)?;
            let mut item = ManifestItem::new_explicit(rsc.src().to_string(), rsc.media_type);
            if self.cover.as_ref().is_some_and(|c| c.id() == rsc.id()) {
                item.props |= ManifestProperties::COVER_IMAGE;
            }
            self.opf.manifest.push(item);
            zip.write_all(&rsc.data)?;
        }

        let spec = self.opf.finish().map_err(|e| error!("{e:?}")).unwrap();

        zip.start_file("EPUB/nav.xhtml", compressed)?;
        write_nav(&mut zip, spec.native_title(), &chunks, &self.sections)?;

        zip.start_file("EPUB/css/epub.css", compressed)?;
        zip.write_all(include_str!("../../epub.css").as_bytes())?;

        zip.start_file("EPUB/package.opf", stored)?;
        spec.write(&mut zip)?;
        let mut w = zip.finish()?;
        w.flush()?;

        Ok(())
    }
}

impl<'a> Default for EpubBuilder<'a> {
    fn default() -> Self {
        Self::new()
    }
}

const CONTAINER_XML: &str = r#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
   <rootfiles>
      <rootfile
          full-path="EPUB/package.opf"
          media-type="application/oebps-package+xml"/>
   </rootfiles>
</container>"#;

fn section_ranges(
    sections: &[(Box<str>, usize)],
) -> Option<impl Iterator<Item = (Option<&str>, usize)>> {
    use std::iter;

    let no_section = (None, 0..sections.first()?.1);
    let main = sections
        .windows(2)
        .map(|w| (Some(&*w[0].0), w[0].1..w[1].1));
    let end = sections
        .last()
        .map(|&(ref t, i)| (Some(&**t), i..usize::MAX))?;
    let ret = iter::once(no_section)
        .chain(main)
        .chain(iter::once(end))
        .filter(|(_, r)| !r.is_empty())
        .map(|(t, r)| (t, r.len()));
    Some(ret)
}

fn chapter_hrefs<'h, 'a>(
    org: &[&'a [Chapter<'h>]],
) -> impl Iterator<Item = (String, &'a Chapter<'h>)> {
    org.into_iter().enumerate().flat_map(|(chunk_idx, &chunk)| {
        chunk.iter().map(move |chapter| {
            (
                format!("chunk_{chunk_idx}.xhtml#{id}", id = chapter.id()),
                chapter,
            )
        })
    })
}

fn write_entry<W: Write>(
    ol: &mut epub::xml::Element<W>,
    href: &str,
    ch: &Chapter,
) -> io::Result<()> {
    ol.mkel("li", [])?
        .mkel("a", [("href", href)])?
        .write_field(EscapeBody(ch.title()))?;
    writeln!(ol)
}

fn write_no_sections<W: Write>(
    nav: &mut epub::xml::Element<W>,
    org: &[&[Chapter]],
) -> io::Result<()> {
    let mut ol = nav.mkel("ol", [])?;
    for (href, ch) in chapter_hrefs(org) {
        write_entry(&mut ol, &href, ch)?;
    }
    Ok(())
}

fn write_sections<'a, W: Write>(
    nav: &mut epub::xml::Element<W>,
    org: &[&[Chapter]],
    sections: impl Iterator<Item = (Option<&'a str>, usize)>,
) -> io::Result<()> {
    let mut ol = nav.mkel("ol", [])?;
    let mut chapters = chapter_hrefs(org);
    for (title, len) in sections {
        if let Some(title) = title {
            let mut li = ol.mkel("li", [])?;
            li.mkel("span", [])?.write_field(EscapeBody(title))?;
            let mut ol = li.mkel("ol", [])?;

            for (href, ch) in (&mut chapters).take(len) {
                write_entry(&mut ol, &href, ch)?;
            }
        } else {
            for (href, ch) in (&mut chapters).take(len) {
                write_entry(&mut ol, &href, ch)?;
            }
        }
    }
    Ok(())
}

fn write_nav<W: Write>(
    w: W,
    title: &str,
    org: &[&[Chapter]],
    sections: &[(Box<str>, usize)],
) -> io::Result<()> {
    let mut toc = XmlSink::new_xhtml(w)?;
    let mut html = toc.mkel(
        "html",
        [
            ("xmlns", "http://www.w3.org/1999/xhtml"),
            ("xmlns:epub", "http://www.idpf.org/2007/ops"),
            ("xml:lang", "en"),
            ("lang", "en"),
        ],
    )?;
    {
        let mut head = html.mkel("head", [])?;
        head.mkel("title", [])?.write_field("Table of Contents")?;
        // head.mkel_selfclosed("link", attrs)
    }
    let mut body = html.mkel("body", [])?;
    let mut nav = body.mkel("nav", [("epub:type", "toc")])?;
    nav.mkel("h2", [])?.write_field(title)?;
    if let Some(sections) = section_ranges(sections) {
        write_sections(&mut nav, org, sections)?
    } else {
        write_no_sections(&mut nav, org)?
    }
    drop(nav);
    drop(body);
    drop(html);
    toc.finish()
}

fn write_chunk(w: impl Write, chunk: &[Chapter]) -> io::Result<()> {
    let mut doc = XmlSink::new_xhtml(w)?;
    {
        let mut html = doc.mkel(
            "html",
            [
                ("xmlns", "http://www.w3.org/1999/xhtml"),
                ("xmlns:epub", "http://www.idpf.org/2007/ops"),
                ("lang", "en"),
                ("xml:lang", "en"),
                (
                    "epub:prefix",
                    "z3998: http://www.daisy.org/z3998/2012/vocab/structure/#",
                ),
            ],
        )?;
        {
            let mut head = html.mkel("head", [])?;
            head.mkel_selfclosed(
                "link",
                [
                    ("href", "css/epub.css"),
                    ("type", "text/css"),
                    ("rel", "stylesheet"),
                ],
            )?;
            head.mkel("title", [])?.write_field("chunk")?;
        }
        let mut body = html.mkel("body", [])?;
        for chapter in chunk {
            writeln!(body, "{}", chapter.xml())?;
        }
    }
    doc.finish()
}
