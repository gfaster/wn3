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

use ahash::{HashSet, HashSetExt};
use anyhow::{anyhow, bail, ensure, Context};
use log::{debug, info, log_enabled, trace};
use regex_lite::{NoExpand, Regex};
use scraper::{ElementRef, Html, Selector, StrTendril};
use serde::Deserialize;

use crate::util::Implies as _;

type SedParseErr = anyhow::Error;
type SedParseRes = std::result::Result<Sed, SedParseErr>;

#[derive(Debug, PartialEq, Eq, Clone)]
enum Op {
    /// effectively a no-op
    Match,
    Delete,
    Print,
    PrintAll,
    Replace(Box<str>),
}

#[derive(Deserialize, Debug, Clone)]
#[serde(try_from = "String")]
pub struct Sed {
    op: Op,
    sel: Option<(Selector, Box<str>)>,
    reg: Option<Regex>,
}

impl Sed {
    pub fn new(s: &str) -> SedParseRes {
        parse_sed(s)
    }

    /// parses a matcher (no directive)
    pub fn new_matcher(s: &str) -> SedParseRes {
        let res = Self::new(s)?;
        if !res.is_matcher() {
            bail!("sed must be matcher")
        }
        Ok(res)
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

    #[must_use]
    pub fn is_matcher(&self) -> bool {
        matches!(self.op, Op::Match)
    }

    #[must_use]
    pub fn is_destructive(&self) -> bool {
        matches!(self.op, Op::Delete | Op::Replace(_))
    }

    fn sel(&self) -> Option<&Selector> {
        self.sel.as_ref().map(|(x, _)| x)
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
                if let Some(sel) = &self.sel() {
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

    /// returns true if any decendent of el matches. Only meaningful if [`Self::is_matcher`]
    /// (always returns false otherwise
    pub fn contains_match(&self, el: &ElementRef) -> bool {
        if !self.is_matcher() {
            return false;
        }
        match (self.sel(), self.reg.as_ref()) {
            (None, None) => unreachable!("malformed routine"),
            (None, Some(r)) => el.text().any(|t| r.is_match(t)),
            (Some(s), None) => el.select(s).next().is_some(),
            (Some(s), Some(r)) => el.select(s).flat_map(|e| e.text()).any(|t| r.is_match(t)),
        }
    }

    /// calls `f` on all elements that selector matches, or on all elements if there is no selector
    fn select_el(&self, el: &ElementRef, mut f: impl FnMut(ElementRef)) {
        if let Some(sel) = &self.sel() {
            el.select(sel).for_each(&mut f)
        } else {
            el.descendent_elements().for_each(f)
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
        if let Some(sel) = &self.sel() {
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
        if let Some(sel) = &self.sel() {
            for el in el.ancestors().flat_map(ElementRef::wrap) {
                if sel.matches(&el) {
                    return Some(el);
                }
            }
            return None;
        }
        Some(*el)
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

    /// applies self to html completely. Note this is *super* expensive, since limitations with the
    /// api means we have to fully serialize and reparse
    pub fn apply_full_expensive(&self, html: &mut Html) {
        trace!("applying expensive sed operation");
        if !self.is_destructive() {
            return;
        }

        match self.op {
            Op::Delete => self.apply_deletes_expensive(html),
            Op::Replace(_) => self.apply_subs_expensive(html),
            Op::Match | Op::Print | Op::PrintAll => unreachable!("non destructive operations"),
        }
    }

    /// helper for [`Self::apply_full_expensive`]
    fn apply_subs_expensive(&self, html: &mut Html) {
        debug_assert!(self.is_sub());

        let reg = self.reg.as_ref().expect("sub needs reg");
        let text_ids: Vec<_>;
        if let Some(sel) = self.sel() {
            let mut visited_els = HashSet::new();
            let mut text_subs = HashSet::new();
            for el in html.select(sel) {
                if !visited_els.insert(el.id()) {
                    continue;
                }
                if el.ancestors().any(|el| visited_els.contains(&el.id())) {
                    continue;
                }
                text_subs.extend(el.descendants().flat_map(|n| {
                    if let scraper::Node::Text(t) = n.value() {
                        reg.is_match(t).then_some(n.id())
                    } else {
                        None
                    }
                }))
            }
            text_ids = text_subs.into_iter().collect();
        } else {
            text_ids = html
                .tree
                .nodes()
                .flat_map(|n| {
                    if let scraper::Node::Text(t) = n.value() {
                        reg.is_match(t).then_some(n.id())
                    } else {
                        None
                    }
                })
                .collect();
        }

        for id in text_ids {
            let scraper::Node::Text(t) = html.tree.get(id).expect("id is always in tree").value()
            else {
                unreachable!("id is always text node")
            };
            let new_text: StrTendril = self.apply_text(&**t).into_owned().into();
            *html.tree.get_mut(id).unwrap().value() =
                scraper::Node::Text(scraper::node::Text { text: new_text });
        }
    }

    /// helper for [`Self::apply_full_expensive`]
    fn apply_deletes_expensive(&self, html: &mut Html) {
        debug_assert!(self.is_del());

        // this routine is woefully inefficient, but it's probably Good Enough since it should
        // never ever be run in a hot loop

        if let Some(reg) = &self.reg {
            // I might be able to improve this depending on the traversal order of select by
            // keeping track of the last removed node and skipping while that matches. That will
            // require a bunch of tests tho
            let mut reg_match_ids = Vec::new();
            for node in html.tree.nodes() {
                if let scraper::Node::Text(t) = node.value() {
                    if reg.is_match(t) {
                        reg_match_ids.push(node.id());
                    }
                }
            }
            let mut reg_ids = HashSet::new();
            for id in reg_match_ids {
                // the reg_match_ids are all text nodes, so we don't visit them
                for node in html.tree.get(id).unwrap().ancestors() {
                    if !reg_ids.insert(node.id()) {
                        break;
                    }
                }
            }

            let sel = self.sel().expect("delete has sel");
            let el_ids: HashSet<_> = html.select(sel).map(|e| e.id()).collect();
            for &id in el_ids.intersection(&reg_ids) {
                html.tree.get_mut(id).unwrap().detach()
            }
        } else {
            let ids: Vec<_> = html
                .select(self.sel().expect("must have either reg or sel"))
                .map(|e| e.id())
                .collect();
            for id in ids {
                html.tree.get_mut(id).unwrap().detach();
            }
        }

        let s = html.html();
        // idk if this is necessary but it might be helpful for appeasing the allocator
        *html = Html::new_fragment();
        *html = Html::parse_document(&s);
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
    let &(_i, opk) = ch.peek().context("empty string")?;
    ensure!(
        "sdpP/;".contains(opk),
        "'{opk}' is not a valid sed operation"
    );
    let i;
    let mut selsep;
    if let '/' | ';' = opk {
        (i, selsep) = ch.next().expect("already peeked");
    } else {
        // since we peek above
        ch.next().expect("already peeked");
        (i, selsep) = ch.next().context("no selector")?;
    }
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
        sel = Some((
            Selector::parse(sel_str).map_err(|e| anyhow!("{e}"))?,
            sel_str.into(),
        ));

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
        ';' | '/' => Op::Match,
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

impl std::fmt::Display for Sed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        'pfx: {
            let op = match self.op {
                Op::Match => break 'pfx,
                Op::Delete => 'd',
                Op::Print => 'p',
                Op::PrintAll => 'P',
                Op::Replace(_) => 's',
            };
            write!(f, "{op}")?;
        }
        if let Some((_, s)) = &self.sel {
            write!(f, ";{s}")?;
        }
        if let Some(r) = &self.reg {
            write!(f, "/{}/", r.as_str())?;
        }
        if let Op::Replace(s) = &self.op {
            write!(f, "{s}/")?;
        }
        Ok(())
    }
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
            "/./",
            ";*/./",
            ";*",
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
            .flat_map(|(c, sed)| sed.err().map(|e| format!("{c} => {e:#}")))
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

    #[test]
    fn idempotent_display() {
        #[track_caller]
        fn case(s: &str) {
            assert_eq!(Sed::new(s).unwrap().to_string(), s);
            let sed = Sed::new(s).unwrap();
            assert_eq!(Sed::new(&sed.to_string()).unwrap(), sed);
        }
        case("d;div.entry-content > p:not(:nth-of-type(4) ~ *)/enjoy/");
        case("s;div.entry-content > p:not(:nth-of-type(4) ~ *)/enjoy/goodbye/");
    }

    #[test]
    fn apply_full_delete() {
        #[track_caller]
        fn case(start: &str, end: &str, sed: &str) {
            let sed = Sed::new(sed).unwrap();
            assert!(sed.is_del());
            let start =
                format!("<!DOCTYPE HTML> <html> <head></head> <body> {start} </body> </html>");
            let end = format!("<!DOCTYPE HTML> <html> <head></head> <body> {end} </body> </html>");
            let mut start = Html::parse_document(&start);
            sed.apply_full_expensive(&mut start);
            let actual = start;
            let expected = Html::parse_document(&end);
            assert_eq!(actual.html(), expected.html());
        }

        case("<p></p>", "", "d;p");
        case("<p></p><p></p>", "", "d;p");
        case(r#"<p id="x"></p><p></p>"#, "<p></p>", "d;p#x");
        case(
            r#"<div><div></div></div><div><div></div></div>"#,
            "<div></div><div></div>",
            "d;div > div",
        );
        case(
            r#"<div><div></div></div><div><div></div></div>"#,
            "",
            "d;body > div",
        );
    }
}
