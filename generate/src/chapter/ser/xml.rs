use crate::chapter::{MapDispJoin, SurroundExt, TagSurround};
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
        writeln!(f, "{title}")?;
        p.map_disp_join("\n", |p| XmlMajor(p)).fmt(f)
    }
}

struct XmlInline<'a>(&'a InlineElement<'a>);
impl Display for XmlInline<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            InlineElement::EnableStyles(s) | InlineElement::DisableStyles(s) if s.is_none() => {
                unreachable!("empty style transition is invalid and should never be created")
            }
            InlineElement::EnableStyles(s) => {
                for el in s.el_iter() {
                    write!(f, "{}", el.open())?
                }
            }
            InlineElement::DisableStyles(s) => {
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
                let disp = TagSurround::new(tag, elms.map_disp_join("", |e| XmlInline(e)));
                disp.fmt(f)
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
