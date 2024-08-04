use std::fmt::Display;

use crate::{
    chapter::{EscapeMd, InlineElement, MajorElement, MapDispJoin, NopDisplay, ParagraphMode},
    Chapter,
};

#[derive(Debug, Clone, Copy)]
pub struct Md;
impl super::SerChapter for Md {
    fn disp<'a>(self, el: &'a Chapter) -> impl Display + 'a {
        MdChapter(el)
    }
}

struct MdChapter<'a>(&'a Chapter<'a>);
impl Display for MdChapter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Chapter { title, p, .. } = self.0;
        let title = EscapeMd(title);
        writeln!(f, "# {title}\n")?;
        p.map_disp_join("\n\n", |p| MdMajor(p)).fmt(f)
    }
}

#[derive(Clone, Copy)]
struct MdInline<'a>(&'a InlineElement<'a>);
impl Display for MdInline<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            InlineElement::EnableStyles(style) | InlineElement::DisableStyles(style) => {
                let surround = match (style.bold, style.italic) {
                    (true, true) => "***",
                    (true, false) => "**",
                    (false, true) => "*",
                    (false, false) => "",
                };
                write!(f, "{surround}")
            }
            InlineElement::Text(text) => {
                let disp = EscapeMd(text);
                write!(f, "{disp}")
            }
            InlineElement::TextOwned(text) => {
                let disp = EscapeMd(text);
                write!(f, "{disp}")
            }
            InlineElement::LineFeed => write!(f, " "),
            InlineElement::ExternalLink(l) => write!(f, "[{}]({})", EscapeMd(&l.text), l.href),
        }
    }
}

#[derive(Clone, Copy)]
struct MdMajor<'a>(&'a MajorElement<'a>);
impl Display for MdMajor<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            MajorElement::Paragraph { style, elms } => {
                let prefix = match style.mode {
                    ParagraphMode::Normal => "",
                    ParagraphMode::BlockQuote => "> ",
                };
                f.write_str(prefix)?;
                elms.map_disp_join(NopDisplay, |el| MdInline(el)).fmt(f)
            }
            MajorElement::ImageResolved(i) => i.display_md().fmt(f),
            MajorElement::HorizLine => "---".fmt(f),
            MajorElement::SceneSep(s) => {
                if s.is_empty() {
                    writeln!(f, "### ◇◇")
                } else {
                    writeln!(f, "### ◇ {s} ◇", s = EscapeMd(s))
                }
            }
            MajorElement::Image(_) => todo!(),
        }
    }
}
