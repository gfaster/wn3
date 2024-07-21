use anyhow::{Context, Result};
use generate::Chapter;
use regex_lite::Regex;
use scraper::{ElementRef, Html, Selector};

use crate::common::{add_basic, is_hr, RuleSet};

pub struct Reigokai {
    next_sel: Selector,
    basic_exclude_sel: Selector,
    title_sel: Selector,
    title_reg: Regex,
    p_sel: Selector,
}

impl Reigokai {
    pub fn new() -> Self {
        Self {
            next_sel: Selector::parse("#main p a:last-of-type").unwrap(),
            basic_exclude_sel: Selector::parse(".sharedaddy,p a,script").unwrap(),
            title_sel: Selector::parse("head > title").unwrap(),
            title_reg: Regex::new("\\w* \u{2013} (.*) \\| Reigokai: Isekai TL").unwrap(),
            p_sel: Selector::parse("body *.entry-content p,hr").unwrap(),
        }
    }

    fn simple_exclude(&self, el: &ElementRef) -> bool {
        for e in el.select(&self.basic_exclude_sel) {
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
    }
}

impl RuleSet for Reigokai {
    fn title(&self, html: &Html) -> String {
        html.select(&self.title_sel).next().and_then(|e| e.text().next()).and_then(|t| self.title_reg.captures(t)).and_then(|c| c.get(1)).map(|m| m.as_str()).unwrap_or("Chapter").to_owned()
    }

    fn next_chapter<'a>(&self, html: &'a Html) -> Option<&'a str> {
        let el = html.select(&self.next_sel).last()?;
        el.attr("href")
    }

    fn parse_multichapter_page<'a>(&self, html: &'a Html) -> Result<Chapter<'a>> {
        todo!()
    }

    fn parse_body<'a>(&self, html: &'a Html, ch: &mut generate::ChapterBuilder<'a>) -> Result<()> {
        let mut it = html.select(&self.p_sel);
        if it.clone().next().context("no paragraphs")?.text().any(|t| t.contains("TLN")) {
            let mut removed = 0;
            while let Some(el) = it.next() {
                removed += 1;
                if self.simple_exclude(&el) {
                    continue
                }
                if is_hr(&el) {
                    if removed > 10 {
                        eprintln!("skipped {removed} paragraphs")
                    }
                    break
                }
            }
        }
        for el in it {
            if self.simple_exclude(&el) {
                continue
            }
            add_basic(ch, el)
        }
        Ok(())
    }
}

