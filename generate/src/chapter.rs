use ahash::{HashMap, HashMapExt};
use anyhow::{ensure, Context};
use url::Url;

use crate::{html_writer::*, image::{Image, ImageId, ResolvedImage}};
use std::{borrow::Cow, fmt::Display, ops::Deref, rc::Rc, sync::Arc};

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
            SpanStyleEl::Footnote => "</aside>",
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
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ParagraphStyle {
    pub mode: ParagraphMode,
    pub align: Align,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParagraphMode {
    #[default]
    Normal,
    BlockQuote,
}

#[derive(Debug)]
enum MajorElement<'a> {
    Paragraph {
        style: ParagraphStyle,
        elms: Vec<InlineElement<'a>>,
    },
    Image(ImageId),
    ImageResolved(Rc<ResolvedImage>),
    SceneSep(Box<str>),
    HorizLine,
}

impl Display for MajorElement<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self, f.alternate()) {
            (MajorElement::Paragraph { style, elms }, true) => {
                let prefix = match style.mode {
                    ParagraphMode::Normal => "",
                    ParagraphMode::BlockQuote => "> ",
                };
                f.write_str(prefix)?;
                let disp = elms.disp_join("");
                disp.fmt(f)
            },
            (MajorElement::Paragraph { style, elms }, false) => {
                let tag = match style.mode {
                    ParagraphMode::Normal => "p",
                    ParagraphMode::BlockQuote => "blockquote",
                };
                let disp = TagSurround::new(tag, elms.disp_join(""));
                disp.fmt(f)
            },
            (MajorElement::ImageResolved(i), _) => i.fmt(f),
            (MajorElement::HorizLine, true) => "---".fmt(f),
            (MajorElement::HorizLine, false) => "<hr />".fmt(f),
            (MajorElement::SceneSep(s), true) => {
                if s.is_empty() {
                    writeln!(f, "### ◇◇")
                } else {
                    writeln!(f, "### ◇ {s} ◇", s = EscapeMd(s))
                }
            },
            (MajorElement::SceneSep(s), false) => {
                let s = EscapeBody(s);
                format_args!("◇ {s} ◇").surround(r#"<h3 class="scene-sep">"#, "</h3>").fmt(f)
            },
            (MajorElement::Image(_), true) => todo!(),
            (MajorElement::Image(_), false) => todo!(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum InlineElement<'a> {
    EnableStyles(SpanStyle),
    DisableStyles(SpanStyle),
    Text(&'a str),
    TextOwned(Box<str>),
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
            InlineElement::TextOwned(t) => t.len(),
        }
    }
}

impl<'a> From<Cow<'a, str>> for InlineElement<'a> {
    fn from(value: Cow<'a, str>) -> Self {
        match value {
            Cow::Borrowed(s) => InlineElement::Text(s),
            Cow::Owned(s) => InlineElement::TextOwned(s.into_boxed_str()),
        }
    }
}

impl Display for InlineElement<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            match self {
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
            }
        } else {
            match self {
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
                Self::TextOwned(txt) => {
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
    pub(crate) rsc: Vec<Rc<ResolvedImage>>,
    p: Vec<MajorElement<'a>>,
}

impl Display for Chapter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Chapter { title, p, .. } = self;
        if f.alternate() {
            let title = EscapeMd(title);
            writeln!(f, "# {title}")?;
            writeln!(f)?;
            p.disp_join("\n\n").fmt(f)?;
        } else {
            let title = EscapeBody(title).surround_tag("h2");
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
        self.p.iter().filter_map(|e| {
            if let MajorElement::Paragraph { elms, .. } = e { Some(elms) } else { None }
        }).map(|elms| elms.iter().map(|e| e.size()).sum::<usize>() + 8).sum::<usize>() + 64
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
    pub(crate) resources_unresolved: HashMap<Arc<str>, Image>,
    pub(crate) resources_resolved: HashMap<ImageId, Rc<ResolvedImage>>,

    current_p: Vec<InlineElement<'a>>,

    complete_p: Vec<MajorElement<'a>>,

    // referenced_resources: HashSet<&'a str>,
}

#[derive(Debug)]
pub struct ChapterBuilderError {
    empty: bool,
    missing_title: bool,
    unresolved_resources: bool,
}

impl ChapterBuilderError {
    fn any(&self) -> bool {
        self.missing_title | self.empty | self.unresolved_resources
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
        if self.unresolved_resources {
            writeln!(f, "\tUnresolved resources")?;
        }
        Ok(())
    }
}

impl std::error::Error for ChapterBuilderError {}

impl<'a> Default for ChapterBuilder<'a> {
    fn default() -> Self {
        Self::new()
    }
}

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
            resources_unresolved: HashMap::new(),
            resources_resolved: HashMap::new(),
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
        self.complete_p.push(MajorElement::Paragraph { elms: spans, style });
        self
    }

    /// adds a horizontal separator (`<hr>`). Implicitly completes the paragraph
    pub fn add_separator(&mut self) -> &mut Self {
        self.paragraph_finish();
        self.complete_p.push(MajorElement::HorizLine);
        self
    }

    /// adds a scene separator with optional heading. Implicitly completes the paragraph
    pub fn add_scene_sep(&mut self, scene: impl Into<Box<str>>) -> &mut Self {
        self.paragraph_finish();
        self.complete_p.push(MajorElement::SceneSep(scene.into()));
        self
    }

    /// adds an image, inline with page flow. Implicitly completes the paragraph
    pub fn add_image(&mut self, img: impl Into<Image>) -> &mut Self {
        self.paragraph_finish();
        let img: Image = img.into();
        self.complete_p.push(MajorElement::Image(img.id()));
        self.resources_unresolved.insert(Arc::clone(img.url()), img);
        self
    }

    /// whether chapter has image resources that would need to be resolved using
    /// [`Self::resolve_resources`]
    pub fn requires_resolution(&self) -> bool {
        !self.resources_unresolved.is_empty()
    }

    /// make sure we have all the images loaded
    pub fn resolve_resources(&mut self, store: &fetch::FetchContext) -> anyhow::Result<()> {
        // PERF: we don't deduplicate here, but probably should?
        self.resources_resolved.reserve(self.resources_unresolved.len());
        for (url, img) in std::mem::take(&mut self.resources_unresolved) {
            if self.resources_resolved.contains_key(&img.id()) {
                continue;
            }
            let url: Url = url.deref().try_into().context("failed to parse url")?;
            let (ty, bytes) = store.fetch(&url).context("failed fetching resource")?;
            ensure!(ty.is_image(), "resolved type {ty:?} is not an image");
            let img = img.resolve_with(ty, bytes);
            self.resources_resolved.insert(img.id(), Rc::new(img));
        }
        // TODO: don't merge same url -> multiple alts
        for el in &mut self.complete_p {
            let MajorElement::Image(id) = el else { continue };
            *el = MajorElement::ImageResolved(
            self.resources_resolved.get(id).context("image was added without registration")?.clone());
        }
        Ok(())
    }

    /// make sure we have all the images loaded - won't touch network
    pub fn resolve_resources_local(&mut self, store: &fetch::FetchContext) -> anyhow::Result<()> {
        // PERF: we don't deduplicate here, but probably should?
        self.resources_resolved.reserve(self.resources_unresolved.len());
        for (url, img) in std::mem::take(&mut self.resources_unresolved) {
            if self.resources_resolved.contains_key(&img.id()) {
                continue;
            }
            let (ty, bytes) = store.fetch_local(&url).context("failed fetching resource")?;
            ensure!(ty.is_image(), "resolved type {ty:?} is not an image");
            let img = img.resolve_with(ty, bytes);
            self.resources_resolved.insert(img.id(), Rc::new(img));
        }
        // TODO: don't merge same url -> multiple alts
        for el in &mut self.complete_p {
            let MajorElement::Image(id) = el else { continue };
            *el = MajorElement::ImageResolved(
            self.resources_resolved.get(id).context("image was added without registration")?.clone());
        }
        Ok(())
    }

    /// note: content will not be trimmed
    pub fn add_text(&mut self, content: impl Into<Cow<'a, str>>) -> &mut Self {
        self.span_style_actualize();
        self.current_p.push(content.into().into());
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
            unresolved_resources: !self.resources_unresolved.is_empty(),
        };
        if error.any() {
            return Err(error);
        }
        Ok(Chapter {
            id: self.id,
            p: self.complete_p,
            title: self.title.unwrap(),
            rsc: self.resources_resolved.into_values().collect(),
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
            <h2>it works</h2>\n\
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
            <h2>multiple paragraphs</h2>\n\
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
            <h2>transitions</h2>\n\
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
