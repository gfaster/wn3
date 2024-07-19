use std::io::{self, prelude::*};
use zip::{write::SimpleFileOptions, ZipWriter};

use crate::{chapter::Chapter, epub::{package::ManifestItem, xml::XmlSink}, html_writer::EscapeBody};

use super::package::{ContributorRole, IdentifierType, OpfBuilder};

pub struct EpubBuilder<'a> {
    opf: OpfBuilder,
    chapters: Vec<Chapter<'a>>,
    chunk_size: usize,
}

impl<'a> EpubBuilder<'a> {
    pub const fn new() -> Self {
        Self {
            opf: OpfBuilder::new(),
            chapters: Vec::new(),
            chunk_size: 0,
        }
    }

    /// set the number of chapters that are combined to a single file via total bytes. If
    /// size is 0, then each chapter will get its own file.
    ///
    /// default is 0
    pub fn set_chunk_size(&mut self, size: usize) -> &mut Self {
        self.chunk_size = size;
        self
    }

    pub fn add_chapter(&mut self, chapter: Chapter<'a>) -> &mut Self {
        self.chapters.push(chapter);
        self
    }

    pub fn set_title(&mut self, title: impl Into<Box<str>>) -> &mut Self {
        self.opf.title.set(title.into());
        self
    }

    pub fn add_author(&mut self, author: impl Into<Box<str>>) -> &mut Self {
        self.opf.contributers.push((ContributorRole::Author, author.into()));
        self
    }

    pub fn add_translator(&mut self, translator: impl Into<Box<str>>) -> &mut Self {
        self.opf.contributers.push((ContributorRole::Translator, translator.into()));
        self
    }

    pub fn add_contributor(&mut self, role: ContributorRole, creator: impl Into<Box<str>>) -> &mut Self {
        self.opf.contributers.push((role, creator.into()));
        self
    }

    pub fn set_publisher(&mut self, publisher: impl Into<Box<str>>) -> &mut Self {
        self.opf.publisher.set(publisher.into());
        self
    }

    pub fn add_identifier(&mut self, ty: IdentifierType, identifier: impl Into<Box<str>>) -> &mut Self {
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
        assert!(!self.chapters.is_empty());

        let mut zip = ZipWriter::new(w);
        let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let compressed = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("mimetype", stored.clone())?;
        zip.write_all(b"application/epub+zip")?;
        zip.add_directory("EPUB", stored.clone())?;
        zip.add_directory("EPUB/css", stored.clone())?;
        zip.add_directory("META-INF", stored.clone())?;
        zip.start_file("META-INF/container.xml", stored.clone())?;
        zip.write_all(CONTAINER_XML.as_bytes())?;

        // chunks here are just splitting the chapters into small enough files
        let chunks: Vec<_> = self.chapters.chunk_by({
            let mut size = self.chapters[0].size();
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
        }).collect();

        self.opf.manifest.push(ManifestItem::new("css/epub.css"));

        self.opf.manifest.push(ManifestItem::new("nav.xhtml"));
        for (i, chunk) in chunks.iter().enumerate() {
            zip.start_file(format!("EPUB/chunk_{i}.xhtml"), compressed.clone())?;
            write_chunk(&mut zip, chunk)?;
            self.opf.manifest.push(ManifestItem::new(format!("chunk_{i}.xhtml")));
        }


        let spec = self.opf.finish().map_err(|e| eprintln!("{e:?}")).unwrap();

        zip.start_file("EPUB/nav.xhtml", compressed.clone())?;
        write_nav(&mut zip, spec.title(), &chunks)?;

        zip.start_file("EPUB/css/epub.css", compressed.clone())?;
        zip.write_all(include_str!("../../epub.css").as_bytes())?;


        zip.start_file("EPUB/package.opf", stored.clone())?;
        spec.write(&mut zip)?;
        zip.flush()?;
        zip.finish()?;

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

fn write_nav(w: impl Write, title: &str, org: &[&[Chapter]]) -> io::Result<()> {
    let mut toc = XmlSink::new_xhtml(w)?;
    let mut html = toc.mkel("html", [
        ("xmlns", "http://www.w3.org/1999/xhtml"),
        ("xmlns:epub", "http://www.idpf.org/2007/ops"),
        ("xml:lang", "en"),
        ("lang", "en"),
    ])?;
    {
        let mut head = html.mkel("head", [])?;
        head.mkel("title", [])?.write_field("Table of Contents")?;
        // head.mkel_selfclosed("link", attrs)
    }
    let mut body = html.mkel("body", [])?;
    let mut nav = body.mkel("nav", [("epub:type", "toc")])?;
    nav.mkel("h1", [])?.write_field(title)?;
    let mut ol = nav.mkel("ol", [])?;
    for (chunk_idx, &chunk) in org.iter().enumerate() {
        for chapter in chunk {
            ol.mkel("li", [])?.mkel("a", [("href", &*format!("chunk_{chunk_idx}.xhtml#{id}", id = chapter.id()))])?.write_field(EscapeBody(chapter.title()))?;
        }
    }
    drop(ol);
    drop(nav);
    drop(body);
    drop(html);
    toc.finish()
}

fn write_chunk(w: impl Write, chunk: &[Chapter]) -> io::Result<()> {
    let mut doc = XmlSink::new_xhtml(w)?;
    {
        let mut html = doc.mkel("html", [
                ("xmlns", "http://www.w3.org/1999/xhtml"), 
                ("xmlns:epub", "http://www.idpf.org/2007/ops"), 
                ("lang", "en"),
                ("xml:lang", "en"),
                ("epub:prefix", "z3998: http://www.daisy.org/z3998/2012/vocab/structure/#"),
        ])?;
        {
            let mut head = html.mkel("head", [])?;
            head.mkel_selfclosed("link", [
                ("href", "css/epub.css"),
                ("type", "text/epub.css"),
                ("rel", "stylesheet")
            ])?;
            head.mkel("title", [])?.write_field("chunk")?;
        }
        let mut body = html.mkel("body", [])?;
        for chapter in chunk {
            body
                .mkel("section", [("id", &*chapter.id().to_string())])?
                .write_field(chapter)?;
        }
    }
    doc.finish()
}
