use crate::html_writer::*;
use std::fmt::Display;

// struct ImageDesc<'a> {
//     pub path: Box<str>,
//     pub alt: &'a str,
//     pub title: &'a str,
// }

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SpanStyle {
    bold: bool,
    italic: bool,
    footnote: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpanStyleEl {
    Bold,
    Italic,
    Footnote,
}

impl SpanStyleEl {
    fn open(self) -> &'static str {
        match self {
            SpanStyleEl::Bold => "<b>",
            SpanStyleEl::Italic => "<i>",
            SpanStyleEl::Footnote => r#"<aside role="doc-footnote">"#,
        }
    }

    fn close(self) -> &'static str {
        match self {
            SpanStyleEl::Bold => "</b>",
            SpanStyleEl::Italic => "</i>",
            SpanStyleEl::Footnote => r#"</aside>"#,
        }
    }
}

impl From<SpanStyleEl> for SpanStyle {
    fn from(value: SpanStyleEl) -> Self {
        match value {
            SpanStyleEl::Bold => SpanStyle {bold: true, ..SpanStyle::none()},
            SpanStyleEl::Italic => SpanStyle {italic: true, ..SpanStyle::none()},
            SpanStyleEl::Footnote => SpanStyle {footnote: true, ..SpanStyle::none()},
        }
    }
}

impl SpanStyle {
    pub const fn none() -> Self {
        SpanStyle { bold: false, italic: false, footnote: false }
    }

    pub const fn is_none(self) -> bool {
        matches!(self, SpanStyle { bold: false, italic: false, footnote: false })
    }

    pub fn el_iter(self) -> impl DoubleEndedIterator<Item = SpanStyleEl> {
        [
            self.bold.then_some(SpanStyleEl::Bold),
            self.italic.then_some(SpanStyleEl::Italic),
            self.footnote.then_some(SpanStyleEl::Footnote),
        ].into_iter().flatten()
    }

    pub const fn bold() -> Self {
        SpanStyle {
            bold: true,
            ..Self::none()
        }
    }

    pub const fn italic() -> Self {
        SpanStyle {
            italic: true,
            ..Self::none()
        }
    }

    pub const fn bold_italic() -> Self {
        SpanStyle {
            bold: true,
            italic: true,
            ..Self::none()
        }
    }

    /// styles needed to be enabled to get to `to`
    const fn additional_needed(self, to: Self) -> Self {
        Self {
            bold: !self.bold & to.bold,
            italic: !self.italic & to.italic,
            footnote: !self.footnote & to.footnote,
        }
    }

    /// styles needed to be disabled to get to `to`
    const fn removals_needed(self, to: Self) -> Self {
        Self {
            bold: self.bold & !to.bold,
            italic: self.italic & !to.italic,
            footnote: self.footnote & !to.footnote,
        }
    }
}

impl FromIterator<SpanStyleEl> for SpanStyle {
    fn from_iter<T: IntoIterator<Item = SpanStyleEl>>(iter: T) -> Self {
        iter.into_iter().fold(SpanStyle::none(), |acc, x| acc + x)
    }
}

impl std::ops::Add for SpanStyle {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            bold: self.bold | rhs.bold,
            italic: self.italic | rhs.italic,
            footnote: self.footnote | rhs.footnote,
        }
    }
}

impl std::ops::AddAssign for SpanStyle {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs
    }
}

impl std::ops::Add<SpanStyleEl> for SpanStyle {
    type Output = Self;

    fn add(self, rhs: SpanStyleEl) -> Self::Output {
        self + Self::from(rhs)
    }
}

impl std::ops::AddAssign<SpanStyleEl> for SpanStyle {
    fn add_assign(&mut self, rhs: SpanStyleEl) {
        *self = *self + rhs
    }
}



#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ParagraphStyle {
    mode: ParagraphMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParagraphMode {
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
            let prefix = match self.style.mode {
                ParagraphMode::Normal => "",
                ParagraphMode::BlockQuote => "> ",
            };
            f.write_str(prefix)?;
            let disp = self.elms.disp_join("");
            disp.fmt(f)
        } else {
            let tag = match self.style.mode {
                ParagraphMode::Normal => "p",
                ParagraphMode::BlockQuote => "blockquote",
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
    LineFeed,
}

impl InlineElement<'_> {
    /// approximate inline size in bytes
    pub fn size(&self) -> usize {
        match self {
            InlineElement::EnableStyles(_) => 8,
            InlineElement::DisableStyles(_) => 8,
            InlineElement::Text(t) => t.len(),
            InlineElement::LineFeed => 6,
        }
    }
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
                InlineElement::LineFeed => write!(f, " "),
            }
        } else {
            match *self {
                Self::EnableStyles(s) | Self::DisableStyles(s) if s.is_none() => {
                    unreachable!("empty style transition is invalid and should never be created")
                },
                Self::EnableStyles(s) => {
                    for el in s.el_iter() {
                        write!(f, "{}", el.open())?
                    }
                }
                Self::DisableStyles(s) => {
                    for el in s.el_iter().rev() {
                        write!(f, "{}", el.close())?
                    }
                }
                Self::Text(txt) => {
                    write!(f, "{}", EscapeBody(txt))?;
                }
                InlineElement::LineFeed => {
                    writeln!(f, "<br />")?;
                },
            };
            Ok(())
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
            let title = EscapeBody(&*title).surround_tag("h2");
            writeln!(f, "{title}")?;
            p.disp_join("\n").fmt(f)?;
        }
        Ok(())
    }
}

impl Chapter<'_> {
    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn id(&self) -> impl Display {
        struct D(u32);
        impl Display for D {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "chapter-{}", self.0)
            }
        }
        D(self.id)
    }

    /// approximate size in bytes - tries to be an overestimate
    pub fn size(&self) -> usize {
        self.p.iter().map(|p| p.elms.iter().map(|e| e.size()).sum::<usize>() + 8).sum::<usize>() + 64
    }
}

#[derive(Debug)]
pub struct ChapterBuilder<'a> {
    id: u32,
    pub title: Option<Box<str>>,

    pub paragraph_style: ParagraphStyle,
    pub span_style: SpanStyle,
    span_style_actual: SpanStyle,
    pub preserve_line_feeds: bool,

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
            preserve_line_feeds: false,
        }
    }

    pub fn title_set(&mut self, s: impl Into<Box<str>>) -> &mut Self {
        self.title = Some(s.into());
        self
    }

    pub fn preserve_line_feeds(&mut self, enable: bool) -> &mut Self {
        self.preserve_line_feeds = enable;
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
        if self.preserve_line_feeds {
            let mut it = content.lines();
            if let Some(first) = it.next() {
                let first = first.trim();
                self.current_p.push(InlineElement::Text(first))
            }
            for rem in it {
                let rem = rem.trim();
                self.current_p.push(InlineElement::LineFeed);
                self.current_p.push(InlineElement::Text(rem));
            }
        } else {
            let content = content.trim();
            self.current_p.push(InlineElement::Text(content));
        }
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
