use anyhow::{ensure, Context, Result};
use wn3::*;
use common::Rules;
use def::BookDef;
use fetch::FetchContext;
use generate::EpubBuilder;
use scraper::Html;


#[tokio::main]
async fn main() -> Result<()> {
    let f = std::fs::read_to_string("cfg.toml").context("failed to open config")?;
    let def: BookDef = toml::from_str(&f).context("failed to parse config")?;
    def.validate().context("failed to validate def")?;

    let rules = Rules::new_il();
    let mut book = generate::EpubBuilder::new();
    let conn = rusqlite::Connection::open("cache.db")?;
    let client = reqwest::ClientBuilder::new()
        .user_agent("wn-scraper3/0.0.1 (github.com/gfaster)")
        .build()
        .unwrap();
    let cx = FetchContext::new(conn, client).unwrap();
    book.set_title(def.title);
    book.add_identifier(generate::epub::IdentifierType::Url, def.url);

    for entry in def.content {
        match entry {
            def::ContentEntry::UrlRange { ruleset_override, exclude_urls, start, end } => {
                if let Err(e) = fetch_range(&cx, &mut book, &rules, &start, &end, &exclude_urls).await.context("fetching urls") {
                    eprintln!("{e:?}")
                }
            },
            def::ContentEntry::Url { ruleset_override, title_override, url } => {
                if let Err(e) = fetch_range(&cx, &mut book, &rules, &url, &url, &[]).await.context("fetching url") {
                    eprintln!("{e:?}")
                }
            },
            def::ContentEntry::Section { section_title } => {
                eprintln!("TODO: use section title ({section_title:?})")
            },
        }
    }

    let mut out = std::fs::OpenOptions::new().write(true).read(false).truncate(true).create(true).open("output.epub")?;
    book.finish(&mut out)?;
    Ok(())
}

async fn fetch_range(cx: &FetchContext, book: &mut EpubBuilder<'_>, rules: &Rules, start: &str, end: &str, excluded: &[String]) -> anyhow::Result<()> {
    let mut prev = None;
    let mut curr = start;
    loop {
        let (ty, val) = cx.fetch(curr).await.context("failed fetching")?;
        ensure!(ty == fetch::MediaType::Html, "{ty:?} is of wrong type");
        let html = std::str::from_utf8(&val).context("not valid utf-8")?;
        let html = Html::parse_document(&html);
        let html = Box::leak(Box::new(html));
        let (ch, next) = rules.parse(html).context("failed to parse")?;
        if let Some(url) = excluded.iter().find(|url| &**url == curr) {
            eprintln!("skipping {url:?}");
        } else {
            book.add_chapter(ch);
        }
        ensure!(prev != next, "url {prev:?} was repeated");
        prev = Some(curr);
        let Some(next) = next else { break };
        if curr == end {
            break
        }
        curr = next
    }
    Ok(())
}

// #[allow(dead_code)]
// fn test_local() {
//     let file = std::env::args().nth(1).unwrap_or_else(|| "il-test.html".into());
//     let html = std::fs::read_to_string(file).unwrap();
//     Rules::new().parse(&html);
// }

