//! # Advanced matching via strings
//!
//! # Examples
//!
//! `s;p.intro/TLN/Note/`
//! = "inside all text matching the CSS selector `p.intro`, replace `TLN` with `Note`"
//!
//! `s/TLN/Note/` = "replace all text matching `TLN` with `Note`"
//!
//! `d;p.intro` = "delete all elements matching selector `p.intro`"
//!
//! `d;p.intro/TLN/`
//! = "delete all elements matching selector `p.intro` with any text matching `TLN`"
//!
//! `p;p.intro/TLN/`
//! = "print all elements matching selector `p.intro` with any text matching `TLN`"
//!
//! `P;p.intro/TLN/`
//! = "print the tree of elements matching selector `p.intro` with any text matching `TLN`"

use std::{borrow::Cow, str::FromStr};

use anyhow::{anyhow, ensure, Context};
use log::{debug, info, log_enabled};
use regex_lite::{NoExpand, Regex};
use scraper::{ElementRef, Selector};
use serde::Deserialize;

use crate::util::Implies as _;

type SedParseErr = anyhow::Error;
type SedParseRes = std::result::Result<Sed, SedParseErr>;

#[derive(Debug, PartialEq, Eq, Clone)]
enum Op {
    Delete,
    Print,
    PrintAll,
    Replace(Box<str>),
}

#[derive(Deserialize, Debug, Clone)]
#[serde(try_from = "String")]
pub struct Sed {
    op: Op,
    sel: Option<Selector>,
    reg: Option<Regex>,
}

impl Sed {
    #[must_use]
    pub fn new(s: &str) -> SedParseRes {
        parse_sed(s).with_context(|| format!("failed to parse sed {s}"))
    }

    #[must_use]
    pub fn is_del(&self) -> bool {
        matches!(self.op, Op::Delete)
    }

    #[must_use]
    pub fn is_sub(&self) -> bool {
        matches!(self.op, Op::Replace(_))
    }

    #[must_use]
    pub fn is_print(&self) -> bool {
        matches!(self.op, Op::Print)
    }

    #[must_use]
    pub fn is_print_all(&self) -> bool {
        matches!(self.op, Op::PrintAll)
    }

    #[must_use]
    pub fn is_print_any(&self) -> bool {
        matches!(self.op, Op::PrintAll | Op::Print)
    }

    /// prints recursively, implicitly checks op
    pub fn print(&self, el: &ElementRef) {
        if !self.is_print_any() {
            return;
        }
        if !log_enabled!(target: "sed", log::Level::Info) {
            return;
        }
        match &self.op {
            Op::Print => {
                let reg = self.reg.as_ref().unwrap();
                if let Some(sel) = &self.sel {
                    el.select(sel)
                        .flat_map(|e| e.text())
                        .flat_map(|t| reg.captures_iter(t))
                        .for_each(|c| info!(target: "sed", "{}", c.get(0).unwrap().as_str()))
                } else {
                    el.text()
                        .flat_map(|t| reg.captures_iter(t))
                        .for_each(|c| info!(target: "sed", "{}", c.get(0).unwrap().as_str()))
                }
            }
            Op::PrintAll => self.select_el(el, |e| info!(target: "sed", "{e:?}")),
            _ => unreachable!(),
        }
    }

    /// calls `f` on all elements that selector matches, or on all elements if there is no selector
    fn select_el(&self, el: &ElementRef, mut f: impl FnMut(ElementRef)) {
        if let Some(sel) = &self.sel {
            el.select(sel).for_each(|el| f(el))
        } else {
            el.descendent_elements().for_each(|el| f(el))
        }
    }

    #[must_use]
    pub fn should_delete(&self, el: &ElementRef) -> bool {
        if !self.is_del() {
            return false;
        }

        self.is_el_match(el)
    }

    /// returns true if element has full match (css and regex)
    #[must_use]
    pub fn is_el_match(&self, el: &ElementRef) -> bool {
        if !self.is_css_match(el) {
            return false;
        }

        if let Some(reg) = &self.reg {
            return el.text().any(|t| reg.is_match(t));
        }

        // no regex and passed css match
        true
    }

    /// returns true if either the css selector matches `el` or if there is no css selector (which
    /// means all elements match)
    ///
    /// If an element is a css match, then all it's children are too
    #[must_use]
    pub fn is_css_match(&self, el: &ElementRef) -> bool {
        if let Some(sel) = &self.sel {
            return sel.matches(el);
        }
        true
    }

    /// returns Some if either the css selector matches `el` or it's parents. always returns Some
    /// if there is no selector.
    ///
    /// If an element is a css match, then all it's children are too
    #[must_use]
    pub fn parent_css_match<'a>(&self, el: &ElementRef<'a>) -> Option<ElementRef<'a>> {
        if let Some(sel) = &self.sel {
            for el in el.ancestors().flat_map(ElementRef::wrap) {
                if sel.matches(&el) {
                    return Some(el);
                }
            }
            return None;
        }
        Some(el.clone())
    }

    /// Apply to `s`, this assumes CSS matches
    // this will be quite hot, so I'm going to try to make it efficient
    #[must_use]
    pub fn apply_text<'a>(&self, s: impl Into<Cow<'a, str>>) -> Cow<'a, str> {
        let Op::Replace(replace) = &self.op else {
            return s.into();
        };
        let Some(reg) = &self.reg else {
            unreachable!("having replace implies regex")
        };
        let s: Cow<str> = s.into();
        let res = reg.replace_all(&s, NoExpand(replace));
        match res {
            Cow::Borrowed(_) => s,
            Cow::Owned(res) => Cow::Owned(res),
        }
    }
}

impl PartialEq for Sed {
    fn eq(&self, other: &Self) -> bool {
        self.op == other.op
            && self.reg.as_ref().map(Regex::as_str) == other.reg.as_ref().map(Regex::as_str)
            && self.sel == other.sel
    }
}

impl Eq for Sed {}

impl FromStr for Sed {
    type Err = SedParseErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Sed::new(s)
    }
}

impl TryFrom<&str> for Sed {
    type Error = SedParseErr;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Sed::new(value)
    }
}

impl TryFrom<String> for Sed {
    type Error = SedParseErr;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Sed::new(&value)
    }
}

/// don't use this directly - use [`Sed::new`] instead
fn parse_sed(s: &str) -> SedParseRes {
    let mut ch = s.char_indices().peekable();
    let (_i, opk) = ch.next().context("empty string")?;
    ensure!("sdpP".contains(opk), "'{opk}' is not a valid sed operation");
    let (i, mut selsep) = ch.next().context("no selector")?;
    ensure!(
        "/;".contains(selsep),
        "expected either '/' or ';' but found '{selsep}'"
    );

    let mut sel = None;
    let mut reg = None;

    // css selector
    let mut start = i + 1;
    if selsep == ';' {
        while let Some((_i, c)) = ch.next_if(|c| c.1 != '/') {
            if c == '\\' {
                // skip escaped characters - this is incomplete but should be fine
                ch.next().context("backslash without escaped character")?;
            }
        }
        // set null for endc since we use it for selsep
        let (endi, endc) = ch.next().unwrap_or((s.len(), '\0'));
        let sel_str = &s[start..endi];
        ensure!(!sel_str.trim().is_empty(), "cannot use empty selector");
        debug!("parsing selector `{sel_str}`");
        sel = Some(Selector::parse(sel_str).map_err(|e| anyhow!("{e}"))?);

        // note that if there is nothing more after this, then start is two past the end
        start = endi + 1;
        selsep = endc;
    }
    // regex
    if selsep == '/' {
        while let Some((_i, c)) = ch.next_if(|c| c.1 != '/') {
            if c == '\\' {
                // skip escaped characters - this is incomplete but should be fine
                ch.next().context("backslash without escaped character")?;
            }
        }
        let (endi, _endc) = ch.next().context("no trailing '/' for regex")?;

        let reg_str = &s[start..endi];
        ensure!(!reg_str.is_empty(), "cannot use empty regex");
        debug!("parsing regex /{reg_str}/");
        reg = Some(Regex::new(reg_str)?);

        start = endi + 1;
    }

    // substitue string
    let mut rep = None;
    if opk == 's' {
        ensure!(reg.is_some(), "substitute requires replacement");

        while let Some((_i, c)) = ch.next_if(|c| c.1 != '/') {
            if c == '\\' {
                // skip escaped characters - this is incomplete but should be fine
                ch.next().context("backslash without escaped character")?;
            }
        }
        let &(endi, _endc) = ch.peek().context("no trailing '/' for replacement")?;

        rep = Some(s[start..endi].to_owned().into_boxed_str());
    }

    let op = match opk {
        'd' => Op::Delete,
        's' => Op::Replace(rep.unwrap()),
        'p' => Op::Print,
        'P' => Op::PrintAll,
        _ => unreachable!("invalid op character ('{opk}') should have been filtered earlier"),
    };

    assert!(
        sel.is_some() || reg.is_some(),
        "this should be checked earlier"
    );
    ensure!(
        (op == Op::Print).implies(reg.is_some()),
        "'p' directive requires regex (try 'P')"
    );

    let ret = Sed { op, sel, reg };

    Ok(ret)
}

#[cfg(test)]
mod tests {
    use scraper::Html;

    use super::*;

    #[test]
    fn it_works() {
        // let _log = crate::util::test_log_level(log::LevelFilter::Debug);
        let fragment = Html::parse_fragment("<div>foo</div>");
        let sed = Sed::new("d;html > div/foo/").unwrap();
        assert!(sed.should_delete(&fragment.root_element().child_elements().next().unwrap()));
    }

    #[test]
    fn should_delete() {
        #[track_caller]
        fn case(html: &str, sed: &str, should: bool) {
            let fragment = Html::parse_fragment(html);
            assert!(fragment.errors.is_empty());
            let sed = Sed::new(sed).unwrap();
            let root = fragment
                .root_element()
                .child_elements()
                .next()
                .expect("should have child");
            assert_eq!(root.value().name(), "div");
            assert!(should == sed.should_delete(&root));
        }
        case("<div></div>", "d;div", true);
        case("<div>foo</div>", "d;div", true);
        case("<div><span></span></div>", "d;div", true);
        case("<div><span>foo</span></div>", "d;div", true);
        case("<div><span>foo</span></div>", "d;div", true);
        case("<div><span>foo</span></div>", "d;div/foo/", true);
        case("<div><span>foo</span></div>", "d;div/bar/", false);
        case("<div><span>foo</span></div>", "d;div/^f/", true);
        case("<div><span>foo</span></div>", "d;div/^o/", false);
        case("<div><span>foo</span></div>", "d;div/foo/", true);
    }

    #[test]
    fn should_delete_adv() {
        #[track_caller]
        fn case(html: &str, sed: &str, should: bool) {
            let fragment = Html::parse_fragment(html);
            assert!(fragment.errors.is_empty());
            let sed = Sed::new(sed).unwrap();
            let sel = Selector::parse("#x").unwrap();
            let el = fragment.select(&sel).next().unwrap();
            assert!(should == sed.should_delete(&el));
        }
        case(r#"<div><span id="x"></span></div>"#, "d;div > span", true);
        case(
            r#"<div><span id="x">foo</span></div>"#,
            "d;div > span",
            true,
        );
        case(
            r#"<div><span id="x">foo</span></div>"#,
            "d;div > span/foo/",
            true,
        );
        case(
            r#"<div><span id="x">foo</span></div>"#,
            "d;div > span/bar/",
            false,
        );
        case(
            r#"<div><span id="x">foo</span></div>"#,
            "d;div > span/o/",
            true,
        );
        case(
            r#"<div><span id="x">foo</span></div>"#,
            "d;div > span/^f/",
            true,
        );
        case(
            r#"<div><span id="x">foo</span></div>"#,
            "d;div > span/^o/",
            false,
        );
        case(
            r#"<div><p></p><span id="x">foo</span></div>"#,
            "d;div > p + span/foo/",
            true,
        );
        case(
            r#"<div><span id="x">foo</span></div>"#,
            "d;div > p + span/foo/",
            false,
        );
    }

    #[test]
    fn parse_fail() {
        let cases = vec![
            "", "/", "s/", "s/./", "d///", "d//", "s", "d", "p/", "p//", "p;div", "P/", "ds//",
            "sd///", "d/(/", "d /(/", "d;/./", "d;",
        ];
        let res: Vec<_> = cases
            .into_iter()
            .map(|c| (c, Sed::new(c)))
            .flat_map(|(c, sed)| sed.ok().map(|_| c))
            .collect();
        assert!(res.is_empty(), "cases incorrectly parsed: {res:#?}");
    }

    #[test]
    fn parse_succ() {
        let cases = vec![
            "s/.//",
            "s/./foo/",
            "s/.*/foo/",
            "s/^.*$?/foo/",
            "s/^.*$?//",
            "d/./",
            "d;div/./",
            "d;div",
            "P;div",
        ];
        let res: Vec<_> = cases
            .into_iter()
            .map(|c| (c, Sed::new(c)))
            .flat_map(|(c, sed)| sed.err().map(|_| c))
            .collect();
        assert!(
            res.is_empty(),
            "cases incorrectly failed to parse: {res:#?}"
        );
    }

    #[test]
    fn replacing() {
        #[track_caller]
        fn case(start: &str, sed: &str, expected: &str) {
            let sed = Sed::new(sed).unwrap();
            assert_eq!(sed.apply_text(start), expected);
        }

        case("TLN asdf", "s/TLN //", "asdf");
        case("TLN asdf", "s/TLN.*$//", "");
        case(" TLN", "s/^TLN//", " TLN");
    }
}
