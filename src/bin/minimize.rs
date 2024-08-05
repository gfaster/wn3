use clap::Parser;
use ego_tree::NodeId;
use markup5ever::namespace_url;
use markup5ever::{
    interface::{tree_builder::TreeSink, NodeOrText},
    ns, LocalName, QualName,
};
use std::collections::HashSet;
use std::path::PathBuf;
use wn3::def::sed::Sed;

use regex_lite::Regex;
use scraper::{Html, Node, Selector};
use wn3::common::Rules;

const BAD_NEXT: &str = "https://example.com/not_next";
const GOOD_NEXT: &str = "https://example.com/next";

fn set_attr(html: &mut Html, id: NodeId, attr: &str, val: &str) {
    let qualname = QualName::new(None, ns!(), LocalName::from(attr));
    let mut node = html.tree.get_mut(id).expect("node id not in tree");
    let Node::Element(el) = node.value() else {
        panic!("node id {id:?} is not an element")
    };
    *el.attrs.get_mut(&qualname).unwrap() = val.into();
}

fn args() -> (Html, ValidateRule) {
    let Args {
        file,
        no_match,
        must_match,
        no_contain,
        must_contain,
        no_next,
        next_url,
        xml,
        invariant_match,
        invariant_contain,
        invariant_html,
        overrider,
    } = Args::parse();
    let f = std::fs::read_to_string(file).unwrap();
    let mut next_must_be = None;
    let mut html = Html::parse_document(&f);
    drop(f);
    for hsed in overrider {
        hsed.apply_full_expensive(&mut html);
    }
    if let Some(next) = next_url {
        next_must_be = Some(true);

        let selector = Selector::parse("a").unwrap();
        let link_ids: Vec<_> = html
            .select(&selector)
            .filter_map(|e| e.attr("href").map(|href| (e.id(), href == next)))
            .collect();
        for (id, is_next) in link_ids {
            let href = if is_next { GOOD_NEXT } else { BAD_NEXT };
            set_attr(&mut html, id, "href", href);
        }
    }
    if no_next {
        next_must_be = Some(false);
    }
    let mut contain_rules =
        Vec::with_capacity(no_contain.len() + must_contain.len() + invariant_contain.len());
    contain_rules.extend(no_contain.into_iter().map(CaseType::FailOmits));
    contain_rules.extend(must_contain.into_iter().map(CaseType::FailHas));
    contain_rules.extend(invariant_contain.into_iter().map(CaseType::InvariantHas));

    let mut regex_rules =
        Vec::with_capacity(no_match.len() + must_match.len() + invariant_match.len());
    regex_rules.extend(no_match.into_iter().map(CaseType::FailOmits));
    regex_rules.extend(must_match.into_iter().map(CaseType::FailHas));
    regex_rules.extend(invariant_match.into_iter().map(CaseType::InvariantHas));

    let serialization_style = if xml {
        SerStyle::Xml
    } else {
        SerStyle::Markdown
    };
    let validate = ValidateRule {
        regex_rules,
        contain_rules,
        next_must_be,
        serialization_style,
        invariant_html,
    };
    (html, validate)
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(name = "HTML_FILE")]
    file: PathBuf,

    /// the generated chapter incorrectly does not match the regex (prefer no-contain for simple
    /// strings)
    #[arg(long, value_parser = Regex::new, value_name = "REGEX")]
    no_match: Vec<Regex>,

    /// the generated chapter incorrectly matches the regex (prefer must-contain for simple
    /// strings)
    #[arg(long, value_parser = Regex::new, value_name = "REGEX")]
    must_match: Vec<Regex>,

    /// the generated chapter correctly matches the regex and must continue to do so (prefer
    /// invariant-contain for simple strings)
    #[arg(long, value_parser = Regex::new, value_name = "REGEX")]
    invariant_match: Vec<Regex>,

    /// the generated chapter incorrectly omits this string (prefer this over no-match)
    #[arg(long, value_name = "STRING")]
    no_contain: Vec<String>,

    /// the generated chapter incorrectly contains this string (prefer this over must-match)
    #[arg(long, value_name = "STRING")]
    must_contain: Vec<String>,

    /// the generated chapter correctly contains the string and must continue to do so (prefer
    /// this over invariant-match)
    #[arg(long, value_name = "STRING")]
    invariant_contain: Vec<String>,

    /// Hsed matcher that must match html
    ///
    /// Basic syntax is `[; <CSS_SELECTORS>][/<REGEX>/]` where at least one is specified
    #[arg(long, value_parser = Sed::new_matcher, value_name = "HSED")]
    invariant_html: Vec<Sed>,

    /// applies an override - see example config
    ///
    /// Note that this works differently from normal overrides as it actually changes the
    /// underlying DOM. This is very slow for the generator, but it's fine here.
    #[arg(long, value_parser = Sed::new, value_name = "HSED")]
    overrider: Vec<Sed>,

    /// the next url is incorrectly found (it should be None)
    #[arg(long, group = "next")]
    no_next: bool,

    /// the next url should be this, but isn't
    #[arg(long, group = "next")]
    next_url: Option<String>,

    /// if the serialization should be done in XML-style instead of markdown
    ///
    /// Note: XML is invalid (ie just a fragment)
    #[arg(long, short)]
    xml: bool,
}

fn main() {
    use std::io::prelude::*;
    use std::path::{Path, PathBuf};

    if !AsRef::<Path>::as_ref("tests/generated").is_dir() {
        panic!("could not read tests/generated dir")
    }

    let (html, validator) = args();

    let rules = Rules::new_il();
    let html = minimize(&validator, html, &rules);

    println!("minimization complete:");
    println!("{}", html.html());
    println!();

    let mut stdout = std::io::stdout();
    let mut buf = String::new();
    let mut p;
    let test_name;
    loop {
        write!(stdout, "test case name: ").unwrap();
        stdout.flush().unwrap();
        buf.clear();
        std::io::stdin().read_line(&mut buf).unwrap();
        let trimmed = buf.trim();
        if trimmed.is_empty() {
            eprintln!("test case name must be non-empty");
            continue;
        }
        if trimmed.starts_with(|c: char| c.is_ascii_digit()) {
            eprintln!("test case cannot start with a digit");
            continue;
        }
        if trimmed.contains(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
            eprintln!("test case {trimmed:?} contains illegal character");
            continue;
        }
        p = PathBuf::from("tests/generated/");
        let f = format!("{trimmed}.rs");
        p.push(f);
        if p.exists() {
            eprintln!("test case {trimmed} already exists!");
            continue;
        }
        test_name = trimmed.to_owned();
        break;
    }
    create_test(&test_name, &validator, &html)
}

fn create_test(test_name: &str, validation: &ValidateRule, html: &Html) {
    use std::io::prelude::*;
    let html = html.html();
    let separate_test_file = html.len() > 1024 * 2;
    if separate_test_file {
        let in_path = format!("tests/generated/{test_name}.input.html");
        std::fs::write(&in_path, &html).unwrap();
    }
    {
        let test_path = format!("tests/generated/{test_name}.rs");
        let mut f = std::fs::File::create_new(&test_path).unwrap();
        if !separate_test_file {
            writeln!(f, r#"const HTML: &str = "{}";"#, html.escape_default()).unwrap();
            writeln!(f).unwrap();
        }
        writeln!(f, "#[test]").unwrap();
        writeln!(f, "fn generated() {{").unwrap();
        if separate_test_file {
            let in_path = format!("tests/generated/{test_name}.input.html");
            writeln!(
                f,
                r#"    let html: &str = &std::fs::read_to_string("{in_path}").unwrap();"#
            )
            .unwrap();
            writeln!(f, r#"    let html: scraper::HTML::parse_document(html);"#).unwrap();
        } else {
            writeln!(f, "    let html = scraper::Html::parse_document(HTML);").unwrap();
        }
        for line in validation.test_code() {
            writeln!(f, "    {line}").unwrap();
        }
        writeln!(f, "}}").unwrap();
        eprintln!("wrote to {test_path}");
        match std::process::Command::new("rustfmt")
            .arg(&test_path)
            .output()
        {
            Ok(_) => (),
            Err(e) => eprintln!("failed to rustfmt: {e}"),
        }
        eprintln!("content of {test_path}:");
        eprintln!("{}", std::fs::read_to_string(test_path).unwrap());
    }
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open("tests/generated/main.rs")
        .unwrap();
    writeln!(f, "mod {test_name};").unwrap();
}

enum SerStyle {
    /// not fully valid xml, just that of a single chapter
    Xml,
    Markdown,
}

enum CaseType<T> {
    /// both the test case failure and success satisfy this
    InvariantHas(T),
    /// what the failing test has, but the passing test does not
    FailHas(T),
    /// what the failing test omits, but should be included
    FailOmits(T),
}

impl<T> CaseType<T> {
    fn inner(&self) -> &T {
        match self {
            CaseType::InvariantHas(r) | CaseType::FailHas(r) | CaseType::FailOmits(r) => r,
        }
    }

    fn check_is_neg(&self) -> bool {
        match self {
            CaseType::InvariantHas(_) | CaseType::FailHas(_) => false,
            CaseType::FailOmits(_) => true,
        }
    }
}

impl<T: CaseRule> CaseType<T> {
    /// note that CaseType doesn't implement CaseRule because CaseType is what does the inverting
    fn check(&self, rendered: &str) -> bool {
        let res = self.inner().satisfies(rendered);
        if self.check_is_neg() {
            !res
        } else {
            res
        }
    }

    fn as_assert(&self) -> String {
        let is_neg = matches!(self, CaseType::FailHas(_));
        let negate = if is_neg { "!" } else { "" };
        let stmt = self.inner().as_text();
        format!(r#"assert!({negate}{stmt});"#)
    }
}

trait CaseRule {
    /// check the positive case (inverted for negation)
    fn satisfies(&self, rendered: &str) -> bool;

    /// what will be printed for the positive case
    ///
    /// the rendered text is `let t: String`
    fn as_text(&self) -> String;

    /// failure message, verb + self, does not need to escape
    fn fail_msg(&self) -> String;
}

impl CaseRule for String {
    fn satisfies(&self, rendered: &str) -> bool {
        rendered.contains(self)
    }

    fn as_text(&self) -> String {
        format!(r#"t.contains("{}")"#, self.escape_default())
    }

    fn fail_msg(&self) -> String {
        format!(r#"contains "{self}""#)
    }
}

impl CaseRule for Regex {
    fn satisfies(&self, rendered: &str) -> bool {
        self.is_match(rendered)
    }

    fn as_text(&self) -> String {
        format!(
            r#"regex_lite::Regex::parse("{}").unwrap().is_match(&t)"#,
            self.to_string().escape_default()
        )
    }

    fn fail_msg(&self) -> String {
        format!(r#"matches /{self}/"#)
    }
}

/// validation rule - somewhat confusing
///
/// we are validating if the *test case still causes a bug*.
///
/// # Example
///
/// If a bad generator output contains the string "ad preferences" in the chapter, the
/// `ValidateRule` would be `must_contain: vec!["ad preferences"]`
struct ValidateRule {
    regex_rules: Vec<CaseType<Regex>>,
    contain_rules: Vec<CaseType<String>>,

    /// matchers that should always succeed on html
    invariant_html: Vec<Sed>,

    /// should have next, url always replaced with `"https://example.com/not_next"`
    ///
    /// the test input transformation is valid if...
    /// - `Some(false)`: generator finds no next url
    /// - `Some(true)`: generator finds next url that is `not_next` (ie the expected wrong url)
    next_must_be: Option<bool>,
    serialization_style: SerStyle,
}

impl ValidateRule {
    fn test_code(&self) -> Vec<String> {
        let mut out = Vec::new();
        out.push("#[allow(unused)]".into());
        out.push(
            r#"let (ch, next) = wn3::common::Rules::new_il().parse(&html).expect("failed to parse");"#
                .into(),
        );

        let ser = match self.serialization_style {
            SerStyle::Xml => r#"format!("{ch}")"#,
            SerStyle::Markdown => r#"format!("{ch:#}")"#,
        };
        out.push(format!("let t = {ser};"));
        out.push(r#"println!("==== begin output ====\n{t}\n====  end output  ====");"#.into());

        for r in &self.contain_rules {
            out.push(r.as_assert());
        }
        for r in &self.regex_rules {
            out.push(r.as_assert());
        }

        out
    }

    fn is_valid(&self, html: &Html, rule: &Rules) -> bool {
        // need to serialize and re-parse because (I suspect) caching internally
        // This may be able to be avoid by adding a no-cache feature to scraper, but I'd have to
        // look more closely at a lot of code
        let html = Html::parse_document(&html.html());
        let Ok((ch, next)) = rule.parse(&html) else {
            return false;
        };
        if let Some(next_must_be) = self.next_must_be {
            match (next_must_be, next) {
                (true, Some("https://example.com/not_next")) | (false, None) => (),
                (false, Some(_)) | (true, _) => return false,
            }
        }

        for r in &self.invariant_html {
            debug_assert!(r.is_matcher());
            if !r.contains_match(&html.root_element()) {
                return false;
            }
        }

        let txt: String = match self.serialization_style {
            SerStyle::Xml => ch.into_iter().map(|ch| ch.xml().to_string()).collect(),
            SerStyle::Markdown => ch.into_iter().map(|ch| ch.md().to_string()).collect(),
        };

        for r in &self.contain_rules {
            if !r.check(&txt) {
                return false;
            };
        }
        for r in &self.regex_rules {
            if !r.check(&txt) {
                return false;
            };
        }

        true
    }

    fn invalid_reasons(&self, html: &Html, rule: &Rules) -> Vec<String> {
        // need to serialize and re-parse because (I suspect) caching internally
        // This may be able to be avoid by adding a no-cache feature to scraper, but I'd have to
        // look more closely at a lot of code
        let html = Html::parse_document(&html.html());
        let Ok((ch, next)) = rule.parse(&html) else {
            return vec!["failed to parse".into()];
        };
        if let Some(next_must_be) = self.next_must_be {
            match (next_must_be, next) {
                (true, Some("https://example.com/not_next")) | (false, None) => (),
                (false, Some(_)) | (true, _) => todo!(),
            }
        }

        let mut ret = Vec::new();

        for r in &self.invariant_html {
            debug_assert!(r.is_matcher());
            if !r.contains_match(&html.root_element()) {
                ret.push(format!("failed invariant: {r}"))
            }
        }

        let txt: String = match self.serialization_style {
            SerStyle::Xml => ch.into_iter().map(|ch| ch.xml().to_string()).collect(),
            SerStyle::Markdown => ch.into_iter().map(|ch| ch.md().to_string()).collect(),
        };
        for r in &self.contain_rules {
            if !r.check(&txt) {
                let neg = if r.check_is_neg() {
                    "negative"
                } else {
                    "positive"
                };
                ret.push(format!("failed {neg} {}", r.inner().fail_msg()))
            };
        }
        for r in &self.regex_rules {
            if !r.check(&txt) {
                let neg = if r.check_is_neg() {
                    "negative"
                } else {
                    "positive"
                };
                ret.push(format!("failed {neg} {}", r.inner().fail_msg()))
            };
        }
        ret
    }

    #[track_caller]
    fn assert_valid(&self, html: &Html, rule: &Rules) {
        assert!(
            self.is_valid(html, rule),
            "html:\n{}\n\noutput:\n{}\n\nfailed patterns: {:#?}",
            html.html(),
            rule.parse(html).map_or_else(
                |_| "FAILED TO PARSE".into(),
                |(ch, _)| ch
                    .into_iter()
                    .map(|ch| ch.md().to_string())
                    .collect::<String>()
            ),
            self.invalid_reasons(html, rule)
        )
    }
}

fn minimize(validation: &ValidateRule, mut html: Html, rule: &Rules) -> Html {
    validation.assert_valid(&html, rule);

    let mut required = HashSet::new();
    let root_el_id = html.root_element().id();
    let root_id = html.tree.root().id();
    assert_eq!(html.root_element().value().name(), "html");
    required.insert(root_el_id);
    required.insert(root_id);

    // deleting nodes that are unneeded
    loop {
        // println!("========================");
        // println!("{}", html.html());
        let Some(el) = html
            .tree
            .root()
            .descendants()
            .find(|n| !required.contains(&n.id()) && !n.value().is_doctype())
        else {
            break;
        };
        let id = el.id();
        let next_sibling = el.next_sibling().map(|e| e.id());
        let parent = el
            .parent()
            .map(|e| e.id())
            .expect("all removable nodes have parents");
        // let before = html.html();
        html.remove_from_parent(&id);
        if validation.is_valid(&html, rule) {
            continue;
        } else {
            // eprintln!("need element {:?}", html.tree.get(id).unwrap().value());
        }
        required.insert(id);
        match next_sibling {
            None => html.append(&parent, NodeOrText::AppendNode(id)),
            Some(s) => html.append_before_sibling(&s, NodeOrText::AppendNode(id)),
        }
        // let after_restore = html.html();
        debug_assert_eq!(
            html.tree.get(id).unwrap().parent().map(|n| n.id()),
            Some(parent)
        );
        // assert_eq!(before, after_restore);
    }
    eprintln!("tree elimination complete");
    validation.assert_valid(&html, rule);
    html = Html::parse_document(&html.html());

    required = HashSet::new();
    required.insert(root_el_id);
    required.insert(root_id);
    let mut children = Vec::new();
    // unwrapping nodes that are unneeded
    loop {
        let Some(el) =
            html.tree.root().descendants().find(|n| {
                !required.contains(&n.id()) && !n.value().is_doctype() && n.has_children()
            })
        else {
            break;
        };
        let id = el.id();
        let parent = el.parent().unwrap().id();
        let next_sibling = el.next_sibling().map(|e| e.id());
        children.clear();
        children.extend(el.children().map(|c| c.id()));
        for &child in &children {
            html.append_before_sibling(&id, NodeOrText::AppendNode(child))
        }
        html.remove_from_parent(&id);
        if validation.is_valid(&html, rule) {
            // eprintln!("don't need element {:?}", html.tree.get(id).unwrap().value());
            continue;
        }
        match next_sibling {
            None => html.append(&parent, NodeOrText::AppendNode(id)),
            Some(s) => html.append_before_sibling(&s, NodeOrText::AppendNode(id)),
        }
        for &child in &children {
            html.append(&id, NodeOrText::AppendNode(child))
        }
        required.insert(id);
    }
    eprintln!("unwrapping complete");
    assert!(validation.is_valid(&html, rule), "");

    let ids: Vec<_> = html
        .root_element()
        .descendent_elements()
        .filter(|e| e.value().classes().count() > 0)
        .map(|e| e.id())
        .collect();
    for id in ids {
        let el = html
            .tree
            .get(id)
            .unwrap()
            .value()
            .as_element()
            .unwrap()
            .clone();
        let qualname = QualName::new(None, ns!(), LocalName::from("class"));
        let mut classes = HashSet::new();
        classes.extend(el.classes());
        for class in el.classes() {
            let node = &mut html.tree.get_mut(id).unwrap();
            let node = node.value();
            let Node::Element(e) = node else { panic!() };
            e.attrs
                .insert(
                    qualname.clone(),
                    classes
                        .iter()
                        .copied()
                        .filter(|&c| c != class)
                        .collect::<Vec<_>>()
                        .join(" ")
                        .into(),
                )
                .unwrap();
            if validation.is_valid(&html, rule) {
                classes.remove(class);
            }
        }
        let node = &mut html.tree.get_mut(id).unwrap();
        let node = node.value();
        let Node::Element(e) = node else { panic!() };
        if classes.is_empty() {
            if let Some(c) = e.attrs.get_mut(&qualname) {
                *c = "".into()
            }
        } else {
            *e.attrs.get_mut(&qualname).unwrap() =
                classes.iter().copied().collect::<Vec<_>>().join(" ").into();
        }
    }
    eprintln!("class stripping complete");
    validation.assert_valid(&html, rule);

    // removing unneeded attributes
    let ids: Vec<_> = html
        .root_element()
        .descendent_elements()
        .map(|e| e.id())
        .collect();
    for id in ids {
        let el = html
            .tree
            .get(id)
            .unwrap()
            .value()
            .as_element()
            .unwrap()
            .clone();
        for attr in el.attrs {
            let node = &mut html.tree.get_mut(id).unwrap();
            let node = node.value();
            let Node::Element(e) = node else { panic!() };
            let prev = e.attrs.remove(&attr.0).unwrap();
            if validation.is_valid(&html, rule) {
                // eprintln!(r#"attr {}="{prev}" is uneeded"#, attr.0.local);
                continue;
            }
            let node = &mut html.tree.get_mut(id).unwrap();
            let node = node.value();
            let Node::Element(e) = node else { panic!() };
            e.attrs.insert(attr.0, prev);
        }
    }
    eprintln!("attribute stripping complete");

    // TODO: attempt to do text substitution
    // sometimes rules are dependant on only a subset of the text. For example, if we wanted to
    // strip out translator notes, we might encounter something like this:
    //
    // "he unsheathed his tanto. (TLN: a tanto is a japanese shortsword)"
    //
    // We don't need the entire note for our test case, we could do with:
    //
    // "(TLN: text)"
    //
    // Currently, the best way to do this is with `sed` after we're done

    validation.assert_valid(&html, rule);

    html
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_impls() {
        let c = "abc".to_owned();
        assert!(c.satisfies("xxabcxx"));
        assert!(c.satisfies("abc"));

        let r = Regex::new("a.c").unwrap();
        assert!(r.satisfies("abc"));
    }

    #[test]
    fn case_type_negations() {
        let r = CaseType::InvariantHas("abc".to_owned());
        assert!(r.check("abc"));
        assert!(!r.check("a c"));
        let r = CaseType::FailHas("abc".to_owned());
        assert!(r.check("abc"));
        assert!(!r.check("a c"));
        let r = CaseType::FailOmits("abc".to_owned());
        assert!(!r.check("abc"));
        assert!(r.check("a c"));
    }
}
