use std::borrow::Cow;

use anyhow::{Context, Result};
use generate::Chapter;
use log::{debug, warn};
use regex_lite::Regex;
use scraper::{ElementRef, Html, Selector};

use crate::{
    common::{ProcessConfig, RuleSet, add_basic, is_hr},
    overrides::OverrideSet,
};

pub struct IlConfig {
    pub strip_fwd_tln: bool,
}

impl Default for IlConfig {
    fn default() -> Self {
        IlConfig {
            strip_fwd_tln: true,
        }
    }
}

pub struct Reigokai {
    next_sel: Selector,
    basic_exclude_sel: Selector,
    scene_sep_reg: Regex,
    title_sel: Selector,
    title_reg: Regex,
    p_sel: Selector,
    cfg: IlConfig,
}

impl Reigokai {
    pub fn new_with_config(cfg: IlConfig) -> Self {
        Self {
            next_sel: Selector::parse("#main p a:last-of-type").unwrap(),
            basic_exclude_sel: Selector::parse(".sharedaddy,p a,script").unwrap(),
            scene_sep_reg: Regex::new(r#"^◇([^◇]*)◇?\s*$"#).unwrap(),
            title_sel: Selector::parse("head > title").unwrap(),
            title_reg: Regex::new(
                "(?:\\w* \u{2013} )?((?:Chapter|\\[?Not|POV|Prologue|Afterword).*) \\| Reigokai: ",
            )
            .unwrap(),
            p_sel: Selector::parse("body div.entry-content > *:is(p,hr,ol,ul)").unwrap(),
            cfg,
        }
    }

    pub fn new() -> Self {
        Self::new_with_config(IlConfig::default())
    }

    fn simple_exclude(&self, el: &ElementRef, overrides: &OverrideSet) -> bool {
        if overrides.replacers().any(|s| s.should_delete(el)) {
            return true;
        }
        for e in el.select(&self.basic_exclude_sel) {
            if e.value().name() == "script" {
                return true;
            }
            if let Some(class) = e.attr("class") {
                if class.contains("sharedaddy") {
                    return true;
                }
            }
            if e.text()
                .any(|t| t.contains("Next Chapter") || t.contains("Previous Chapter"))
            {
                return true;
            }
        }
        false
    }

    fn simple_title<'a>(&self, html: &'a Html) -> &'a str {
        html.select(&Selector::parse("head > title").unwrap())
            .next()
            .and_then(|t| t.text().next())
            .unwrap_or("unknown chapter")
    }
}

impl RuleSet for Reigokai {
    fn title(&self, html: &Html) -> String {
        let mut title = None;
        html.select(&self.title_sel)
            .next()
            .and_then(|e| e.text().next())
            .and_then(|t| {
                title = Some(t);
                self.title_reg.captures(t)
            })
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .unwrap_or_else(|| {
                if let Some(title) = title {
                    warn!("page `{title}` does not match title regex")
                } else {
                    warn!("page has no title")
                }
                "Chapter"
            })
            .to_owned()
    }

    fn next_chapter<'a>(&self, html: &'a Html) -> Option<Cow<'a, str>> {
        let el = html.select(&self.next_sel).last()?;
        el.attr("href").map(Cow::Borrowed)
    }

    fn parse_multichapter_page<'a>(&self, _html: &'a Html) -> Result<Chapter<'a>> {
        todo!()
    }

    fn parse_body<'a>(
        &self,
        html: &'a Html,
        overrides: &OverrideSet,
        ch: &mut generate::ChapterBuilder<'a>,
    ) -> Result<()> {
        let filter = |el: &ElementRef| !self.simple_exclude(el, overrides);
        let mut it = html.select(&self.p_sel).filter(filter);
        let first = it.clone().next().context("no paragraphs")?;
        let pcfg = ProcessConfig {
            br_is_paragraph: false,
        };
        if self.cfg.strip_fwd_tln
            && first
                .text()
                .any(|t| t.contains("TLN") || t.contains("Sponsored"))
        {
            let mut removed = 0;
            debug!("removing prefix TLN");
            while let Some(el) = it.next() {
                removed += 1;
                if self.simple_exclude(&el, overrides) {
                    continue;
                }
                if is_hr(&el) {
                    if removed > 10 {
                        warn!(
                            "removed perhaps too many paragraphs: resetting in {}",
                            self.simple_title(html)
                        );
                        it = html.select(&self.p_sel).filter(filter);
                    }
                    break;
                }
                // if removed > 25 && first.text().any(|t| t == "Sponsored Chapter!") {
                //     // probably just no hr below sponsed chapter (maybe look at
                //     // right-align)
                //     it = html.select(&self.p_sel).filter(filter);
                //     let first = it.next().and_then(|e| e.text().next());
                //     debug!("removed a bunch after sponsored chapter with no newl: resetting");
                //     ensure!(first == Some("Sponsored Chapter!"));
                //     break;
                // }
                if removed > 25 {
                    warn!(
                        "removing a whole lot: resetting in {}",
                        self.simple_title(html)
                    );
                    it = html.select(&self.p_sel).filter(filter);
                    break;
                }
            }
        }
        for el in it {
            if let Some(sep) = el
                .text()
                .next()
                .and_then(|txt| self.scene_sep_reg.captures(txt))
            {
                let scene = sep.get(1).map_or("", |m| m.as_str().trim());
                ch.add_scene_sep(scene);
            } else {
                add_basic(ch, el, overrides, &pcfg)
            }
        }
        Ok(())
    }
}
