use std::borrow::Cow;

use anyhow::{Result, ensure};
use generate::Chapter;
use log::warn;
use regex_lite::Regex;
use scraper::{Html, Selector};

use crate::{
    common::{ProcessConfig, RuleSet, add_basic},
    overrides::OverrideSet,
};

pub struct Rule {
    next_sel: Selector,
    scene_sep_reg: Regex,
    title_sel: Selector,
    p_sel: Selector,
}

impl Rule {
    pub fn new() -> Self {
        Self {
            next_sel: Selector::parse("a.c-pager__item--next").unwrap(),
            scene_sep_reg: Regex::new(r#"^\s*◇+\s*$"#).unwrap(),
            title_sel: Selector::parse("h1.p-novel__title--rensai").unwrap(),
            p_sel: Selector::parse("div.p-novel__text > *").unwrap(),
        }
    }
}

impl RuleSet for Rule {
    fn title(&self, html: &Html) -> String {
        html.select(&self.title_sel)
            .next()
            .and_then(|e| e.text().next())
            .unwrap_or_else(|| {
                warn!("page has no title");
                "〇〇話"
            })
            .to_owned()
    }

    fn next_chapter<'a>(&self, html: &'a Html) -> Option<Cow<'a, str>> {
        let el = html.select(&self.next_sel).next()?;
        let href = el.attr("href")?;

        let href = if href.starts_with("//") {
            // implicit protocol
            Cow::Owned(format!("https:{href}"))
        } else if href.starts_with("/") {
            // Not sure if applying this ruleset will always be on ncode.syosetu, but I'll assume
            // that for now
            Cow::Owned(format!("https://ncode.syosetu.com{href}"))
        } else {
            Cow::Borrowed(href)
        };

        Some(href)
    }

    fn parse_multichapter_page<'a>(&self, _html: &'a Html) -> Result<Chapter<'a>> {
        unimplemented!()
    }

    fn parse_body<'a>(
        &self,
        html: &'a Html,
        overrides: &OverrideSet,
        ch: &mut generate::ChapterBuilder<'a>,
    ) -> Result<()> {
        let it = html
            .select(&self.p_sel)
            .filter(|el| !overrides.should_delete(el));
        let pcfg = ProcessConfig {
            br_is_paragraph: false,
        };
        let mut empty = true;
        for el in it {
            if el
                .text()
                .next()
                .is_some_and(|txt| self.scene_sep_reg.is_match(txt))
            {
                ch.add_scene_sep("◇");
            } else {
                empty = false;
                add_basic(ch, el, overrides, &pcfg)
            }
        }
        ensure!(!empty, "no paragraphs");
        Ok(())
    }
}
