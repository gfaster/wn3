//! common rules that should rarely be overriden

use ego_tree::NodeRef;
use generate::{chapter::SpanStyle, Chapter, ChapterBuilder};
use regex_lite::Regex;
use scraper::{node::Element, ElementRef, Html, Node};
use anyhow::{Context, Result};

pub trait RuleSet {
    fn title(&self, html: &Html) -> String;
    fn next_chapter<'a>(&self, html: &'a Html) -> Option<&'a str>;
    fn parse_body<'a>(&self, html: &'a Html, ch: &mut ChapterBuilder<'a>) -> Result<()>;
    fn parse_multichapter_page<'a>(&self, _html: &'a Html) -> Result<Chapter<'a>> {
        todo!()
    }
}

pub struct Rules {
    inner: Box<dyn RuleSet>,
}

impl Rules {
    pub fn new_il() -> Self {
        Rules {
            inner: Box::new(crate::il::Reigokai::new())
        }
    }

    pub fn parse<'a>(&self, html: &'a Html) -> Result<(Chapter<'a>, Option<&'a str>)> {
        let mut ch = ChapterBuilder::new();
        let title = self.inner.title(&html);
        ch.title_set(title.clone());
        self.inner.parse_body(html, &mut ch).with_context(|| format!("invalid chapter: {title}"))?;

        let next = self.inner.next_chapter(&html);
        let ch = ch.finish().with_context(|| format!("invalid chapter: {title}"))?;
        println!("{ch:#}\n");
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
pub fn add_basic<'a>(ch: &mut ChapterBuilder<'a>, el: ElementRef<'a>) {
    descend(ch, *el)
}

fn descend<'a>(ch: &mut ChapterBuilder<'a>, el: NodeRef<'a, Node>) {
    match el.value() {
        scraper::Node::Document => (),
        scraper::Node::Fragment => (),
        scraper::Node::Doctype(_) => (),
        scraper::Node::Comment(_) => (),
        scraper::Node::Text(txt) => {
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
                        descend(ch, child);
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
