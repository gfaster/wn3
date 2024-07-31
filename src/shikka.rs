use anyhow::{Context, Result};
use generate::Chapter;
use scraper::{ElementRef, Html, Selector};

use crate::{common::{add_basic, RuleSet}, overrides::OverrideSet};

pub struct ShikkaConfig {
}

impl Default for ShikkaConfig {
    fn default() -> Self {
        ShikkaConfig { 
        }
    }
}

pub struct Rule {
    next_sel: Selector,
    basic_exclude_sel: Selector,
    title_sel: Selector,
    p_sel: Selector,
    cfg: ShikkaConfig,
}

impl Rule {
    pub fn new_with_config(cfg: ShikkaConfig) -> Self {
        Self {
            next_sel: Selector::parse(".entry-content > p > a").unwrap(),
            basic_exclude_sel: Selector::parse(".sharedaddy,p a,script").unwrap(),
            title_sel: Selector::parse("h1.wp-block-post-title").unwrap(),
            p_sel: Selector::parse("body *.entry-content p,hr,figure").unwrap(),
            cfg,
        }
    }

    pub fn new() -> Self {
        Self::new_with_config(ShikkaConfig::default())
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

impl RuleSet for Rule {
    fn title(&self, html: &Html) -> String {
        html.select(&self.title_sel).next().and_then(|e| e.text().next()).unwrap_or("Chapter").to_owned()
    }

    fn next_chapter<'a>(&self, html: &'a Html) -> Option<&'a str> {
        let el = html.select(&self.next_sel).last()?;
        if !el.text().any(|t| t.contains("Next")) {
            return None
        }
        el.attr("href")
    }

    fn parse_multichapter_page<'a>(&self, _html: &'a Html) -> Result<Chapter<'a>> {
        todo!()
    }

    fn parse_body<'a>(&self, html: &'a Html, overrides: &OverrideSet, ch: &mut generate::ChapterBuilder<'a>) -> Result<()> {
        let _ = self.cfg;
        let filter = |el: &ElementRef| !self.simple_exclude(el);
        let it = html.select(&self.p_sel).filter(filter);
        let _first = it.clone().next().context("no paragraphs")?;
        for el in it {
            if self.simple_exclude(&el) {
                continue
            }
            add_basic(ch, el, overrides)
        }
        Ok(())
    }
}

