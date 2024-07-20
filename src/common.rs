//! common rules that should rarely be overriden

use ego_tree::NodeRef;
use generate::{chapter::SpanStyle, Chapter, ChapterBuilder};
use scraper::{node::Element, ElementRef, Html, Node, Selector};
use anyhow::{Context, Result};

type FnExclude = Box<dyn Fn(&ElementRef) -> bool>;
type FnNext = Box<dyn Fn(&Html) -> Option<&str>>;
type FnTitle = Box<dyn Fn(&Html) -> String>;
pub struct Rules {
    paragraphs: Selector,
    exclude: FnExclude,
    next_chapter: FnNext,
    title: FnTitle
}

fn mk_next_chapter_il() -> FnNext {
    let candidates = Selector::parse("#main p a:last-of-type").unwrap();
    Box::new(move |html| {
        let el = html.select(&candidates).last()?;
        el.attr("href")
    })
}

fn mk_exclude_il() -> FnExclude {
    let candidates = Selector::parse(".sharedaddy,p a,script").unwrap();
    Box::new(move |el| {
        for e in el.select(&candidates) {
            if e.value().name() == "script" {
                return true
            }
            if let Some(class) = e.attr("class") {
                if class.contains("sharedaddy") {
                    return true
                }
            }
            if e.text().any(|t| t.contains("Next Chapter") || t.contains("Previous Chapter")) {
                return true
            }
        }
        false
    })
}

fn mk_title_il() -> FnTitle {
    let sel = Selector::parse("head > title").unwrap();
    Box::new(move |el| {
        el.select(&sel).next().and_then(|e| e.text().next()).unwrap_or("Chapter").to_owned()
    })
}

impl Rules {
    pub fn new() -> Self {
        Rules {
            paragraphs: Selector::parse("body *.entry-content p,hr").unwrap(),
            exclude: mk_exclude_il(),
            next_chapter: mk_next_chapter_il(),
            title: mk_title_il(),
        }
    }

    pub fn parse<'a>(&self, html: &'a Html) -> Result<(Chapter<'a>, Option<&'a str>)> {
        let mut ch = ChapterBuilder::new();
        ch.title_set((self.title)(&html));

        for el in html.select(&self.paragraphs) {
            if (self.exclude)(&el) {
                continue
            }
            Rules::descend(&mut ch, *el);
            ch.paragraph_finish();
            // for txt in el.text() {
            //     println!("{txt}")
            // }
        }
        let next = (self.next_chapter)(&html);
        let ch = ch.finish().context("invalid chapter")?;
        // println!("{ch:#}\n");
        Ok((ch, next))
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
                    _ => {
                        let prev_style = ch.span_style;
                        if is_italics_tag(e) {
                            ch.span_style += SpanStyle::italic();
                        }
                        if is_bold_tag(e) {
                            ch.span_style += SpanStyle::bold();
                        }
                        for child in el.children() {
                            Rules::descend(ch, child);
                        }
                        ch.span_style_set(prev_style);
                    }
                }
            },
            scraper::Node::ProcessingInstruction(_) => (),
        }
    }
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
