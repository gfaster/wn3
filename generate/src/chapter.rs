use crate::html_writer::*;
use std::{collections::HashSet, fmt::Display};

// struct ImageDesc<'a> {
//     pub path: Box<str>,
//     pub alt: &'a str,
//     pub title: &'a str,
// }

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SpanStyle {
    bold: bool,
    italic: bool,
}

impl SpanStyle {
    pub fn is_none(self) -> bool {
        self == Self::default()
    }

    pub const fn bold() -> Self {
        SpanStyle {
            bold: true,
            italic: false,
        }
    }

    pub const fn italic() -> Self {
        SpanStyle {
            bold: false,
            italic: true,
        }
    }

    pub const fn bold_italic() -> Self {
        SpanStyle {
            bold: true,
            italic: true,
        }
    }

    /// styles needed to be enabled to get to `to`
    fn additional_needed(self, to: Self) -> Self {
        Self {
            bold: !self.bold & to.bold,
            italic: !self.italic & to.italic,
        }
    }

    /// styles needed to be disabled to get to `to`
    fn removals_needed(self, to: Self) -> Self {
        Self {
            bold: self.bold & !to.bold,
            italic: self.italic & !to.italic,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParagraphStyle {
    #[default]
    Normal,
    BlockQuote,
}

#[derive(Debug)]
pub struct Paragraph<'a> {
    style: ParagraphStyle,
    elms: Vec<InlineElement<'a>>,
}

impl Display for Paragraph<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            let prefix = match self.style {
                ParagraphStyle::Normal => "",
                ParagraphStyle::BlockQuote => "> ",
            };
            f.write_str(prefix)?;
            let disp = self.elms.disp_join("");
            disp.fmt(f)
        } else {
            let tag = match self.style {
                ParagraphStyle::Normal => "p",
                ParagraphStyle::BlockQuote => "blockquote",
            };
            let disp = TagSurround::new(tag, self.elms.disp_join(""));
            disp.fmt(f)
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum InlineElement<'a> {
    EnableStyles(SpanStyle),
    DisableStyles(SpanStyle),
    Text(&'a str),
}

impl Display for InlineElement<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            match *self {
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
            }
        } else {
            let content = match *self {
                Self::EnableStyles(SpanStyle {
                    bold: true,
                    italic: true,
                }) => "<b><i>",
                Self::EnableStyles(SpanStyle {
                    bold: true,
                    italic: false,
                }) => "<b>",
                Self::EnableStyles(SpanStyle {
                    bold: false,
                    italic: true,
                }) => "<i>",
                Self::DisableStyles(SpanStyle {
                    bold: true,
                    italic: true,
                }) => "</i></b>",
                Self::DisableStyles(SpanStyle {
                    bold: true,
                    italic: false,
                }) => "</b>",
                Self::DisableStyles(SpanStyle {
                    bold: false,
                    italic: true,
                }) => "</i>",
                Self::EnableStyles(SpanStyle {
                    bold: false,
                    italic: false,
                })
                | Self::DisableStyles(SpanStyle {
                    bold: false,
                    italic: false,
                }) => unreachable!("empty style transition is invalid"),
                Self::Text(txt) => {
                    return write!(f, "{}", EscapeBody(txt));
                }
            };
            write!(f, "{content}")
        }
    }
}

#[derive(Debug)]
pub struct Chapter<'a> {
    id: u32,
    title: Box<str>,
    p: Vec<Paragraph<'a>>,
}

impl Display for Chapter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Chapter { title, p, .. } = self;
        if f.alternate() {
            let title = EscapeMd(&*title);
            writeln!(f, "# {title}")?;
            writeln!(f)?;
            p.disp_join("\n\n").fmt(f)?;
        } else {
            let title = EscapeBody(&*title).surround_tag("h1");
            writeln!(f, "{title}")?;
            p.disp_join("\n").fmt(f)?;
        }
        Ok(())
    }
}

impl Chapter<'_> {
    pub fn id(&self) -> impl Display {
        struct D(u32);
        impl Display for D {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "chapter-{}", self.0)
            }
        }
        D(self.id)
    }
}

#[derive(Debug)]
pub struct ChapterBuilder<'a> {
    id: u32,
    pub title: Option<Box<str>>,

    pub paragraph_style: ParagraphStyle,
    pub span_style: SpanStyle,
    span_style_actual: SpanStyle,

    current_p: Vec<InlineElement<'a>>,

    complete_p: Vec<Paragraph<'a>>,

    // referenced_resources: HashSet<&'a str>,
}

#[derive(Debug)]
pub struct ChapterBuilderError {
    empty: bool,
    missing_title: bool,
}

impl ChapterBuilderError {
    fn any(&self) -> bool {
        self.missing_title | self.empty
    }
}

impl std::fmt::Display for ChapterBuilderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.any() {
            writeln!(f, "Unknown chapter error (this is a bug)")?;
            return Ok(());
        }
        writeln!(f, "Chapter invalid:")?;
        if self.empty {
            writeln!(f, "\tNo content")?;
        }
        if self.missing_title {
            writeln!(f, "\tMissing title")?;
        }
        Ok(())
    }
}

impl std::error::Error for ChapterBuilderError {}

impl<'a> ChapterBuilder<'a> {
    pub fn new() -> Self {
        use std::sync::atomic::*;
        static ID_CNT: AtomicU32 = AtomicU32::new(0);
        Self {
            id: ID_CNT.fetch_add(1, Ordering::Relaxed),
            title: Default::default(),
            paragraph_style: Default::default(),
            span_style: Default::default(),
            span_style_actual: Default::default(),
            current_p: Default::default(),
            complete_p: Default::default(),
        }
    }

    pub fn title_set(&mut self, s: impl Into<Box<str>>) -> &mut Self {
        self.title = Some(s.into());
        self
    }

    fn span_style_actualize(&mut self) -> &mut Self {
        let current = self.span_style_actual;
        let new = self.span_style;
        let removals = current.removals_needed(new);
        if !removals.is_none() {
            self.current_p.push(InlineElement::DisableStyles(removals))
        }
        let additional = current.additional_needed(new);
        if !additional.is_none() {
            self.current_p.push(InlineElement::EnableStyles(additional))
        }
        self.span_style_actual = new;
        self
    }

    pub fn span_style_set(&mut self, style: SpanStyle) -> &mut Self {
        self.span_style = style;
        self
    }

    pub fn span_style_reset(&mut self) -> &mut Self {
        self.span_style = SpanStyle::default();
        self
    }

    pub fn paragraph_style_set(&mut self, style: ParagraphStyle) -> &mut Self {
        self.paragraph_style = style;
        self
    }

    /// completes the paragraph, implicitly resets style. no-op if no spans have been added.
    pub fn paragraph_finish(&mut self) -> &mut Self {
        self.span_style_reset();
        self.span_style_actualize();
        if self.current_p.is_empty() {
            return self;
        }
        let spans = std::mem::take(&mut self.current_p);
        let style = std::mem::take(&mut self.paragraph_style);
        self.complete_p.push(Paragraph { elms: spans, style });
        self
    }

    pub fn add_text(&mut self, content: &'a str) -> &mut Self {
        self.span_style_actualize();
        self.current_p.push(InlineElement::Text(content));
        self
    }

    pub fn add_text_styled(&mut self, content: &'a str, style: SpanStyle) -> &mut Self {
        let prev_style = self.span_style;
        self.span_style = style;
        self.add_text(content);
        self.span_style = prev_style;
        self
    }

    pub fn finish(mut self) -> Result<Chapter<'a>, ChapterBuilderError> {
        self.paragraph_finish();
        let error = ChapterBuilderError {
            missing_title: self.title.is_none(),
            empty: self.complete_p.is_empty(),
        };
        if error.any() {
            return Err(error);
        }
        Ok(Chapter {
            id: self.id,
            p: self.complete_p,
            title: self.title.unwrap(),
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn it_works() {
        let mut builder = ChapterBuilder::new();
        builder
            .title_set("it works")
            .add_text("hello, ")
            .add_text_styled("world", SpanStyle::bold())
            .add_text("!");
        let chapter = builder.finish().unwrap();
        let expected = "\
            <h1>it works</h1>\n\
            <p>hello, <b>world</b>!</p>";
        assert_eq!(format!("{chapter}"), expected);
    }

    #[test]
    fn multiple_paragraphs() {
        let mut builder = ChapterBuilder::new();
        builder
            .title_set("multiple paragraphs")
            .add_text("hello, ")
            .add_text("world")
            .add_text_styled("!", SpanStyle::bold())
            .paragraph_finish()
            .add_text("paragraph 2")
            .paragraph_finish()
            .add_text("paragraph 3");
        let chapter = builder.finish().unwrap();
        let expected = "\
            <h1>multiple paragraphs</h1>\n\
            <p>hello, world<b>!</b></p>\n\
            <p>paragraph 2</p>\n\
            <p>paragraph 3</p>";
        assert_eq!(format!("{chapter}"), expected);
    }

    #[test]
    fn transitions() {
        let mut builder = ChapterBuilder::new();
        builder
            .title_set("transitions")
            .add_text("aaa")
            .add_text_styled("bbb", SpanStyle::bold_italic())
            .span_style_set(SpanStyle::bold())
            .add_text_styled("ccc", SpanStyle::bold())
            .add_text_styled("ddd", SpanStyle::italic())
            .add_text("eee")
            .add_text("fff")
            .span_style_reset()
            .add_text("ggg")
            .add_text("hhh");
        let chapter = builder.finish().unwrap();
        let expected = "\
            <h1>transitions</h1>\n\
            <p>aaa<b><i>bbb</i>ccc</b><i>ddd</i><b>eeefff</b>ggghhh</p>";
        assert_eq!(format!("{chapter}"), expected);
    }

    #[test]
    fn markdown() {
        let mut builder = ChapterBuilder::new();
        builder
            .title_set("markdown")
            .add_text("hello, ")
            .add_text("world")
            .span_style_set(SpanStyle::bold())
            .add_text("!")
            .paragraph_finish()
            .add_text("paragraph 2")
            .paragraph_finish()
            .add_text("paragraph 3");
        let chapter = builder.finish().unwrap();
        let expected = "\
            # markdown\n\n\
            hello, world**!**\n\n\
            paragraph 2\n\n\
            paragraph 3";
        assert_eq!(format!("{chapter:#}"), expected);
    }
}
