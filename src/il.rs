use anyhow::{bail, ensure, Context, Result};
use generate::Chapter;
use log::{debug, warn};
use regex_lite::Regex;
use scraper::{ElementRef, Html, Selector};

use crate::{
    common::{add_basic, is_hr, RuleSet},
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
                "(?:\\w* \u{2013} )?((?:Chapter|\\[?Not|POV|Prologue).*) \\| Reigokai: Isekai TL",
            )
            .unwrap(),
            p_sel: Selector::parse("body *.entry-content p,hr").unwrap(),
            cfg,
        }
    }

    pub fn new() -> Self {
        Self::new_with_config(IlConfig::default())
    }

    fn simple_exclude(&self, el: &ElementRef) -> bool {
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

    fn next_chapter<'a>(&self, html: &'a Html) -> Option<&'a str> {
        let el = html.select(&self.next_sel).last()?;
        el.attr("href")
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
        let filter = |el: &ElementRef| !self.simple_exclude(el);
        let mut it = html.select(&self.p_sel).filter(filter);
        let first = it
            .clone()
            .find(|el| !self.simple_exclude(el))
            .context("no paragraphs")?;
        if self.cfg.strip_fwd_tln
            && first
                .text()
                .any(|t| t.contains("TLN") || t.contains("Sponsored"))
        {
            let mut removed = 0;
            debug!("removing prefix TLN");
            while let Some(el) = it.next() {
                removed += 1;
                if self.simple_exclude(&el) {
                    continue;
                }
                if is_hr(&el) {
                    if removed > 10 {
                        let title = self.title(html);
                        // wm 333 has no hr, maybe want to make this as part of cfg.toml
                        if title == "Chapter 333" {
                            it = html.select(&self.p_sel).filter(filter);
                            break;
                        }
                        if removed > 25 && first.text().any(|t| t == "Sponsored Chapter!") {
                            // probably just no hr below sponsed chapter (maybe look at
                            // right-align)
                            it = html.select(&self.p_sel).filter(filter);
                            let first = it.next().and_then(|e| e.text().next());
                            ensure!(first == Some("Sponsored Chapter!"));
                            break;
                        }
                        // extra long note wm 200
                        if title.contains("Chapter 200") {
                            ensure!(
                                removed == 13,
                                "expected 13 skipped chapters for chapter 200 but found {removed}"
                            );
                            break;
                        }
                        // extra long note tsuki 370
                        if title.contains("Chapter 370") {
                            ensure!(
                                removed == 11,
                                "expected 11 skipped chapters for chapter 370 but found {removed}"
                            );
                            break;
                        }
                        bail!("skipped {removed} paragraphs")
                    }
                    break;
                }
                if removed > 25 && first.text().any(|t| t == "Sponsored Chapter!") {
                    // probably just no hr below sponsed chapter (maybe look at
                    // right-align)
                    it = html.select(&self.p_sel).filter(filter);
                    let first = it.next().and_then(|e| e.text().next());
                    debug!("removed a bunch after sponsored chapter with no newl: resetting");
                    ensure!(first == Some("Sponsored Chapter!"));
                    break;
                }
                if removed > 25 {
                    if overrides.is_empty() {
                        warn!("removing a whole lot with no overrides: resetting")
                    } else {
                        debug!("removed a whole bunch with overrides: resetting")
                    }
                    it = html.select(&self.p_sel).filter(filter);
                    break;
                }
            }
        }
        for el in it {
            if self.simple_exclude(&el) {
                continue;
            }
            if let Some(sep) = el
                .text()
                .next()
                .and_then(|txt| self.scene_sep_reg.captures(txt))
            {
                let scene = sep.get(1).map_or("", |m| m.as_str().trim());
                ch.add_scene_sep(scene);
            } else {
                add_basic(ch, el, overrides)
            }
        }
        Ok(())
    }
}
