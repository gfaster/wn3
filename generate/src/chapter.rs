use std::fmt::Display;
use crate::html_writer::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SpanStyle {
    bold: bool,
    italic: bool,
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
    spans: Vec<Span<'a>>,
}

impl Display for Paragraph<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            let prefix = match self.style {
                ParagraphStyle::Normal => "",
                ParagraphStyle::BlockQuote => "> ",
            };
            f.write_str(prefix)?;
            let disp = self.spans.disp_join("");
            disp.fmt(f)
        } else {
            let tag = match self.style {
                ParagraphStyle::Normal => "p",
                ParagraphStyle::BlockQuote => "blockquote",
            };
            let disp = TagSurround::new(tag, self.spans.disp_join(""));
            disp.fmt(f)
        }
    }
}

impl Paragraph<'_> {
    /// calculates the approximate size in bytes
    pub fn size(&self) -> usize {
        self.spans.iter().map(|s| s.content.len()).sum()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Span<'a> {
    style: SpanStyle,
    content: &'a str
}

impl Display for Span<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            let pre_ws = &self.content[..(self.content.len() - self.content.trim_start().len())];
            let post_ws = &self.content[self.content.trim_end().len()..];
            if pre_ws.len() + post_ws.len() > self.content.len() {
                // must be all whitespace
                return Ok(())
            }
            let trimmed = &self.content[pre_ws.len()..(self.content.len() - post_ws.len())];
            let surround = match (self.style.bold, self.style.italic) {
                (true, true) => "***",
                (true, false) => "**",
                (false, true) => "*",
                (false, false) => "",
            };
            let disp = EscapeMd(trimmed).surround(surround, surround);
            write!(f, "{pre_ws}{disp}{post_ws}")
        } else {
            let tags: &[&str] = match (self.style.bold, self.style.italic) {
                (true, true) => &["b", "i"],
                (true, false) => &["b"],
                (false, true) => &["i"],
                (false, false) => &[],
            };
            let disp = EscapeBody(self.content).surround_tag(tags);
            write!(f, "{disp}")
        }
    }
}

#[derive(Debug)]
pub struct Chapter<'a> {
    title: Box<str>,
    p: Vec<Paragraph<'a>>
}

impl Display for Chapter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Chapter { title, p } = self;
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

#[derive(Default, Debug)]
pub struct ChapterBuilder<'a> {
    pub title: Option<Box<str>>,
    pub span_style: SpanStyle,
    pub paragraph_style: ParagraphStyle,
    current_p: Vec<Span<'a>>,

    complete_p: Vec<Paragraph<'a>>,
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
            return Ok(())
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
        Self::default()
    }

    pub fn title_set(&mut self, s: impl Into<Box<str>>) -> &mut Self {
        self.title = Some(s.into());
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
        self.span_style = SpanStyle::default();
        if self.current_p.is_empty() {
            return self
        }
        let spans = std::mem::take(&mut self.current_p);
        let style = std::mem::take(&mut self.paragraph_style);
        self.complete_p.push(Paragraph { spans, style });
        self
    }

    pub fn add_span(&mut self, content: &'a str) -> &mut Self {
        let new = Span {
            style: self.span_style,
            content,
        };
        self.span_style = SpanStyle::default();
        self.current_p.push(new);
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
            p: self.complete_p,
            title: self.title.unwrap()
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
            .add_span("hello, ")
            .span_style_set(SpanStyle { bold: true, ..SpanStyle::default() })
            .add_span("world")
            .add_span("!");
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
            .add_span("hello, ")
            .add_span("world")
            .span_style_set(SpanStyle { bold: true, ..SpanStyle::default() })
            .add_span("!")
            .paragraph_finish()
            .add_span("paragraph 2")
            .paragraph_finish()
            .add_span("paragraph 3");
        let chapter = builder.finish().unwrap();
        let expected = "\
            <h1>multiple paragraphs</h1>\n\
            <p>hello, world<b>!</b></p>\n\
            <p>paragraph 2</p>\n\
            <p>paragraph 3</p>";
        assert_eq!(format!("{chapter}"), expected);
    }

    #[test]
    fn markdown() {
        let mut builder = ChapterBuilder::new();
        builder
            .title_set("markdown")
            .add_span("hello, ")
            .add_span("world")
            .span_style_set(SpanStyle { bold: true, ..SpanStyle::default() })
            .add_span("!")
            .paragraph_finish()
            .add_span("paragraph 2")
            .paragraph_finish()
            .add_span("paragraph 3");
        let chapter = builder.finish().unwrap();
        let expected = "\
            # markdown\n\n\
            hello, world**!**\n\n\
            paragraph 2\n\n\
            paragraph 3";
        assert_eq!(format!("{chapter:#}"), expected);
    }
}
