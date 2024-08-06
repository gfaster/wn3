use crate::chapter::{MapDispJoin, NopDisplay, SurroundExt};
use std::fmt::Display;

use crate::{
    chapter::{EscapeBody, InlineElement, MajorElement, ParagraphMode},
    Chapter,
};

#[derive(Debug, Clone, Copy)]
pub struct Xml;
impl super::SerChapter for Xml {
    fn disp<'a>(self, el: &'a Chapter) -> impl Display + 'a {
        XmlChapter(el)
    }
}

struct XmlChapter<'a>(&'a Chapter<'a>);
impl Display for XmlChapter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Chapter { title, p, .. } = self.0;
        let title = EscapeBody(title).surround_tag("h2");
        writeln!(
            f,
            r#"<section epub:type="chapter" id="{id}">"#,
            id = self.0.id()
        )?;
        writeln!(f, "{title}")?;
        writeln!(f, "{}", p.map_disp_join('\n', |p| XmlMajor(p)))?;
        write!(f, "</section>")
    }
}

struct XmlInline<'a>(&'a InlineElement<'a>);
impl Display for XmlInline<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            InlineElement::EnableStyles(s) => {
                debug_assert!(
                    !s.is_none(),
                    "empty style transition is invalid and should never be created"
                );
                for el in s.el_iter() {
                    write!(f, "{}", el.open())?
                }
            }
            InlineElement::DisableStyles(s) => {
                debug_assert!(
                    !s.is_none(),
                    "empty style transition is invalid and should never be created"
                );
                for el in s.el_iter().rev() {
                    write!(f, "{}", el.close())?
                }
            }
            InlineElement::Text(txt) => {
                write!(f, "{}", EscapeBody(txt))?;
            }
            InlineElement::TextOwned(txt) => {
                write!(f, "{}", EscapeBody(txt))?;
            }
            InlineElement::LineFeed => {
                writeln!(f, "<br />")?;
            }
            InlineElement::ExternalLink(l) => {
                write!(f, r#"<a href="{}">{}</a>"#, l.href, l.text)?;
            }
        };
        Ok(())
    }
}

struct XmlMajor<'a>(&'a MajorElement<'a>);
impl Display for XmlMajor<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            MajorElement::Paragraph { style, elms } => {
                let tag = match style.mode {
                    ParagraphMode::Normal => "p",
                    ParagraphMode::BlockQuote => "blockquote",
                };

                elms.map_disp_join(NopDisplay, |e| XmlInline(e))
                    .surround_tag(tag)
                    .fmt(f)
            }
            MajorElement::ImageResolved(i) => i.display_xml().fmt(f),
            MajorElement::HorizLine => "<hr />".fmt(f),
            MajorElement::SceneSep(s) => {
                let s = EscapeBody(s);
                format_args!("◇ {s} ◇")
                    .surround(r#"<h3 class="scene-sep">"#, "</h3>")
                    .fmt(f)
            }
            MajorElement::Image(_) => todo!(),
        }
    }
}
