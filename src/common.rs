//! common rules that should rarely be overriden

use std::borrow::Cow;

use ego_tree::NodeRef;
use fetch::FetchContext;
use generate::{chapter::SpanStyle, image::Image, Chapter, ChapterBuilder};
use log::warn;
use regex_lite::Regex;
use scraper::{node::Element, ElementRef, Html, Node};
use anyhow::{Context, Result};

use crate::overrides::OverrideSet;

pub trait RuleSet {
    fn title(&self, html: &Html) -> String;
    fn next_chapter<'a>(&self, html: &'a Html) -> Option<&'a str>;
    fn parse_body<'a>(&self, html: &'a Html, overrides: &OverrideSet, ch: &mut ChapterBuilder<'a>) -> Result<()>;
    fn parse_multichapter_page<'a>(&self, _html: &'a Html) -> Result<Chapter<'a>> {
        todo!()
    }
}

pub struct Rules {
    inner: Box<dyn RuleSet>,
}

impl Rules {
    pub fn new(ruleset: impl RuleSet + 'static) -> Self {
        Rules {
            inner: Box::new(ruleset),
        }
    }

    pub fn new_il() -> Self {
        Self::new(crate::il::Reigokai::new())
    }

    pub fn new_shikka() -> Self {
        Self::new(crate::shikka::Rule::new())
    }

    pub fn parse<'a>(&self, html: &'a Html) -> Result<(Chapter<'a>, Option<&'a str>)> {
        self.parse_with_overrides(html, &OverrideSet::empty(), None)
    }

    pub fn parse_with_overrides<'a>(&self, html: &'a Html, overrides: &OverrideSet<'_>, store: Option<&FetchContext>) -> Result<(Chapter<'a>, Option<&'a str>)> {
        let mut ch = ChapterBuilder::new();
        let title = if let Some(title) = &overrides.title {
            title.to_owned()
        } else {
            self.inner.title(&html)
        };
        ch.title_set(title.clone());
        self.inner.parse_body(html, overrides, &mut ch).with_context(|| format!("invalid chapter: {title}"))?;

        let next = self.inner.next_chapter(&html);
        if ch.requires_resolution() {
            let store = store.context("chapter has images but no fetch context was provided")?;
            ch.resolve_resources(store).context("failed to resolve resources")?;
        }
        let ch = ch.finish().with_context(|| format!("invalid chapter: {title}"))?;
        // println!("{ch:#}\n");
        Ok((ch, next))
    }
}

/// basic processing of "normal" blocks
///
/// does:
/// - text of `<p>` recursively, and ends paragraphs
/// - handles styling
/// - handles `<hr>` and similar horizontal separators
/// - converts `<br>` tags to LF for setting-specific handling
pub fn add_basic<'a>(ch: &mut ChapterBuilder<'a>, el: ElementRef<'a>, overrides: &OverrideSet) {
    descend(ch, *el, overrides)
}

fn descend<'a>(ch: &mut ChapterBuilder<'a>, el: NodeRef<'a, Node>, overrides: &OverrideSet) {
    match el.value() {
        scraper::Node::Document => (),
        scraper::Node::Fragment => (),
        scraper::Node::Doctype(_) => (),
        scraper::Node::Comment(_) => (),
        scraper::Node::Text(txt) => {
            let txt = overrides.replacers().fold(Cow::from(&**txt), |acc, sed| sed.apply(acc));
            ch.add_text(txt);
        },
        scraper::Node::Element(e) => {
            match e.name() {
                "hr" => {
                    ch.add_separator();
                },
                "br" => {
                    ch.add_text("\n");
                }
                "img" => {
                    let Some(src) = e.attr("src") else {
                        warn!(target: "parsing", "image {e:?} has no src");
                        return
                    };
                    let src = src.split_once('?').map_or(src, |(base, _query)| base);
                    let alt = e.attr("alt").map(|alt| alt.to_owned());
                    let mut img = Image::new(src);
                    img.alt = alt;
                    ch.add_image(img);
                }
                "script" => (),
                _ => {
                    let prev_style = ch.span_style;
                    if is_italics_tag(e) {
                        ch.span_style += SpanStyle::italic();
                    }
                    if is_bold_tag(e) {
                        ch.span_style += SpanStyle::bold();
                    }
                    for child in el.children() {
                        descend(ch, child, overrides);
                    }
                    ch.span_style_set(prev_style);
                    if e.name() == "p" {
                        ch.paragraph_finish();
                    }
                }
            }
        },
        scraper::Node::ProcessingInstruction(_) => (),
    }
}

pub fn is_hr(el: &ElementRef) -> bool {
    if el.value().name() == "hr" {
        return true
    }
    if let Some(txt) = el.text().next() {
        thread_local! { static HR: Regex = Regex::new("^[\u{2014}\u{2013}=-]+$").unwrap(); }
        return HR.with(|r| r.is_match(txt.trim()))
    }
    false
}

fn is_italics_tag(el: &Element) -> bool {
    if el.name() == "i" || el.name() == "em" {
        return true
    }

    false
}

fn is_bold_tag(el: &Element) -> bool {
    if el.name() == "b" {
        return true
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use scraper::Selector;

    macro_rules! telref {
        ($frag:expr, $pat:expr) => {
            Html::parse_fragment($frag).select(&Selector::parse($pat).unwrap()).next().unwrap()
        };
    }

    #[test]
    fn is_hr_works() {
        assert!(is_hr(&telref!("<p>-</p>", "p")));
        assert!(is_hr(&telref!("<p>--</p>", "p")));
        assert!(is_hr(&telref!("<p>---</p>", "p")));
        assert!(is_hr(&telref!("<p>===</p>", "p")));
        assert!(is_hr(&telref!("<p>——-</p>", "p")));
        assert!(is_hr(&telref!("<p>——- </p>", "p")));
        assert!(is_hr(&telref!("<p>——–</p>", "p")));
        assert!(is_hr(&telref!("<p>—-</p>", "p")));
        assert!(is_hr(&telref!("<p>—</p>", "p")));
        assert!(!is_hr(&telref!("<p>—Great Forest—</p>", "p")));
        assert!(!is_hr(&telref!("<p></p>", "p")));
    }
}
