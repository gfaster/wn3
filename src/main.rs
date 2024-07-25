use ahash::HashMap;
use anyhow::{bail, ensure, Context, Result};
use url::Url;
use wn3::{def::Section, overrides::OverrideTracker, *};
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
    book.add_identifier(generate::epub::IdentifierType::Url, def.homepage.as_str());
    if let Some(tl) = def.translator {
        book.add_translator(tl);
    }

    let sections: HashMap<_, _> = def.sections.into_iter().map(|Section { title, start }| (start, title)).collect();
    let mut overrides = OverrideTracker::new(def.overrides);

    for entry in def.content {
        match entry {
            def::UrlSelection::Range { start, end } => {
                if let Err(e) = fetch_range(&cx, &mut book, &rules, start, end, &sections, &mut overrides).await.context("fetching urls") {
                    eprintln!("{e:?}")
                }
            },
            def::UrlSelection::Url(url) => {
                if let Err(e) = fetch_range(&cx, &mut book, &rules, url.clone(), url, &sections, &mut overrides).await.context("fetching url") {
                    eprintln!("{e:?}")
                }
            },
            def::UrlSelection::List(list) => {
                for url in list {
                    if let Err(e) = fetch_range(&cx, &mut book, &rules, url.clone(), url, &sections, &mut overrides).await.context("fetching url") {
                        eprintln!("{e:?}")
                    }
                }
            }
        }
    }

    let mut out = std::fs::OpenOptions::new().write(true).read(false).truncate(true).create(true).open("output.epub")?;
    book.finish(&mut out)?;
    Ok(())
}

async fn fetch_range(cx: &FetchContext, book: &mut EpubBuilder<'_>, rules: &Rules, start: Url, end: Url, sections: &HashMap<Url, String>, track: &mut OverrideTracker) -> anyhow::Result<()> {
    ensure!(start.scheme() == end.scheme(), "start and end must be on the same scheme");
    ensure!(start.host_str() == end.host_str(), "start and end must be on the same host");
    let mut prev = None;
    let mut curr = start;
    loop {
        if let Some(section) = sections.get(&curr) {
            eprintln!("TODO: handle section {section}")
        }
        ensure!(curr.scheme() == "https" || curr.scheme() == "file", "url {curr} does not have expected scheme");
        let (ty, val) = cx.fetch(&curr).await.context("failed fetching")?;
        ensure!(ty == fetch::MediaType::Html, "{ty:?} is of wrong type");
        let html = std::str::from_utf8(&val).context("not valid utf-8")?;
        let html = Html::parse_document(&html);
        let html = Box::leak(Box::new(html));
        let overrides = track.with_url(&curr);
        let (ch, next) = rules.parse_with_overrides(html, &overrides).context("failed to parse")?;
        let next = if let Some(next) = next {
            Some(Url::parse(next).context("invalid url")?)
        } else {
            None
        };
        book.add_chapter(ch);
        ensure!(prev != next, "url {prev:?} was repeated");
        if curr == end {
            break
        }
        let Some(next) = next else { bail!("expected more urls (up until {end}) but found no next after {curr}") };
        prev = Some(curr);
        ensure!(next.host_str() == end.host_str(), "tried to go to different host for next url: {next}");
        curr = next
    }
    Ok(())
}

