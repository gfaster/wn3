use markup5ever::namespace_url;
use markup5ever::{interface::{tree_builder::TreeSink, NodeOrText}, ns, LocalName, QualName};
use std::collections::HashSet;

use regex_lite::Regex;
use scraper::{Html, Node};
use wn3::common::Rules;

#[allow(dead_code)]
enum Mode {
    Match,
    NoMatch,
}

fn args() -> (Regex, Mode, String) {
    let mut it = std::env::args();
    it.next();
    let mut m = None;
    let mut r = None;
    let mut f = None;

    while let Some(a) = it.next() {
        match &*a {
            "--match" => {
                assert!(r.is_none(), "can only specify one pattern");
                let pat = it.next().expect("--match requires pattern");
                r = Some(Regex::new(&pat).expect("invalid pattern"));
                m = Some(Mode::Match);
            },
            "--no-match" => {
                todo!("idk how I want to implement this");
                // assert!(r.is_none(), "can only specify one pattern");
                // let pat = it.next().expect("--no-match requires pattern");
                // r = Some(Regex::new(&pat).expect("invalid pattern"));
                // m = Some(Mode::NoMatch);
            },
            _ => {
                assert!(f.is_none(), "can only specify one file");
                assert!(a.ends_with(".html"), "can only specify html files");
                f = Some(a);
            }
        }
    }

    (
        r.unwrap(),
        m.expect("requires either --match or --no-match"),
        f.expect("requires file"),
    )
}

fn main() {
    use std::io::prelude::*;
    use std::path::{PathBuf, Path};

    if !AsRef::<Path>::as_ref("tests/generated").is_dir() {
        panic!("could not read tests/generated dir")
    }

    let (regex, _mode, src_file) = args();
    let src_file = std::fs::read_to_string(&src_file).expect("could not open src file");

    let rules = Rules::new();
    let html = minimize(&src_file, &regex, &rules);

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
        break
    }
    create_test(&test_name, &regex, &html)
}

fn create_test(test_name: &str, r: &Regex, html: &Html) {
    use std::io::prelude::*;

    {
        let in_path = format!("tests/generated/{test_name}.input.html");
        std::fs::write(&in_path, &html.html()).unwrap();
    }
    {
        let test_path = format!("tests/generated/{test_name}.rs");
        let mut f = std::fs::File::create_new(&test_path).unwrap();
        let r = r.to_string();
        let r = r.escape_default();
        writeln!(f, "use crate::support::match_test_v1 as match_test;\n").unwrap();
        writeln!(f, "#[test]").unwrap();
        writeln!(f, "fn should_not_match() {{").unwrap();
        writeln!(f, r###"    match_test("{test_name}", r##"{r}"##)"###).unwrap();
        writeln!(f, "}}").unwrap();
        eprintln!("wrote to {test_path}");
    }
    let mut f = std::fs::OpenOptions::new().append(true).open("tests/generated/main.rs").unwrap();
    writeln!(f, "mod {test_name};").unwrap();
}

fn minimize(html: &str, r: &Regex, rule: &Rules) -> Html {
    let mut html_string = html.to_owned();
    let mut html = Html::parse_document(&html_string);
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
        let Some(el) = html.tree.root().descendants().find(|n| !required.contains(&n.id()) && !n.value().is_doctype()) else { break };
        let id = el.id();
        let next_sibling = el.next_sibling().map(|e| e.id());
        let parent = el.parent().map(|e| e.id()).expect("all removable nodes have parents");
        // let before = html.html();
        html.remove_from_parent(&id);
        let new_html_string = html.html();
        if is_valid(&new_html_string, r, rule) {
            html_string = new_html_string;
            continue
        } else {
            // eprintln!("need element {:?}", html.tree.get(id).unwrap().value());
        }
        required.insert(id);
        match next_sibling {
            None => html.append(&parent, NodeOrText::AppendNode(id)),
            Some(s) => html.append_before_sibling(&s, NodeOrText::AppendNode(id)),
        }
        // let after_restore = html.html();
        debug_assert_eq!(html.tree.get(id).unwrap().parent().map(|n| n.id()), Some(parent));
        // assert_eq!(before, after_restore);
    }
    eprintln!("tree elimination complete");
    html_string.shrink_to_fit();
    html = Html::parse_document(&html_string);
    required = HashSet::new();
    required.insert(root_el_id);
    required.insert(root_id);
    let mut children = Vec::new();
    // unwrapping nodes that are unneeded
    loop {
        let Some(el) = html.tree.root().descendants().find(|n| !required.contains(&n.id()) && !n.value().is_doctype() && n.has_children()) else { break };
        let id = el.id();
        let parent = el.parent().unwrap().id();
        let next_sibling = el.next_sibling().map(|e| e.id());
        children.clear();
        children.extend(el.children().map(|c| c.id()));
        for &child in &children {
            html.append_before_sibling(&id, NodeOrText::AppendNode(child))
        }
        html.remove_from_parent(&id);
        if is_valid(&html.html(), r, rule) {
            // eprintln!("don't need element {:?}", html.tree.get(id).unwrap().value());
            continue
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

    let ids: Vec<_> = html.root_element().descendent_elements().filter(|e| e.value().classes().count() > 0).map(|e| e.id()).collect();
    for id in ids {
        let el = html.tree.get(id).unwrap().value().as_element().unwrap().clone();
        let qualname = QualName::new(None, ns!(), LocalName::from("class"));
        let mut classes = HashSet::new();
        classes.extend(el.classes());
        for class in el.classes() {
            let node = &mut html.tree.get_mut(id).unwrap();
            let node = node.value();
            let Node::Element(e) = node else { panic!() };
            e.attrs.insert(qualname.clone(), classes.iter().map(|&c| c).filter(|&c| c != class).collect::<Vec<_>>().join(" ").into()).unwrap();
            if is_valid(&html.html(), r, rule) {
                classes.remove(class);
            }
        }
    }
    eprintln!("class stripping complete");

    // removing unneeded attributes
    let ids: Vec<_> = html.root_element().descendent_elements().map(|e| e.id()).collect();
    for id in ids {
        let el = html.tree.get(id).unwrap().value().as_element().unwrap().clone();
        for attr in el.attrs {
            let node = &mut html.tree.get_mut(id).unwrap();
            let node = node.value();
            let Node::Element(e) = node else { panic!() };
            let prev = e.attrs.remove(&attr.0).unwrap();
            if is_valid(&html.html(), r, rule) {
                // eprintln!(r#"attr {}="{prev}" is uneeded"#, attr.0.local);
                continue
            }
            let node = &mut html.tree.get_mut(id).unwrap();
            let node = node.value();
            let Node::Element(e) = node else { panic!() };
            e.attrs.insert(attr.0, prev);
        }
    }
    eprintln!("attribute stripping complete");

    assert!(is_valid(&html.html(), r, rule));

    html
}



// for whatever reason, we have to parse again for changes to show properly
fn is_valid(html: &str, r: &Regex, rule: &Rules) -> bool {
    // use std::sync::atomic::*;
    // static A: AtomicU64 = AtomicU64::new(1);
    // eprintln!("check #{}", A.fetch_add(1, Ordering::Relaxed));

    let html = Html::parse_document(&html);
    rule.parse(&html).is_ok_and(|(ch, _nxt)| {
        let ch = ch.to_string();
        // println!("{ch}");
        r.is_match(&ch)
    })
}
