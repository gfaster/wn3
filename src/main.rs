use ahash::HashMap;
use anyhow::{bail, ensure, Context, Result};
use log::{error, info, warn};
use url::Url;
use wn3::{def::Section, overrides::OverrideTracker, *};
use common::Rules;
use def::BookDef;
use fetch::FetchContext;
use generate::{image::Image, EpubBuilder};
use scraper::Html;

mod logger;


fn main() -> Result<()> {
    logger::init().unwrap();
    let f = std::fs::read_to_string("cfg.toml").context("failed to open config")?;
    let def: BookDef = toml::from_str(&f).context("failed to parse config")?;
    def.validate().context("failed to validate def")?;

    let args: Vec<_> = std::env::args().collect();
    let rules = if let Some(s) = args.iter().find_map(|a| a.strip_prefix("--rules=")) {
        match s {
            "il" => Rules::new_il(),
            "shikka" => Rules::new_shikka(),
            _ => bail!("unknown ruleset {s}")
        }
    } else {
        Rules::new_il()
    };
    let mut book = generate::EpubBuilder::new();
    let conn = rusqlite::Connection::open("cache.db")?;
    let client = ureq::AgentBuilder::new().https_only(true).user_agent("wn-scraper3/0.0.1 (github.com/gfaster)").build();
    let cx = FetchContext::new(conn, client).unwrap();
    book.set_title(def.title);
    book.add_identifier(generate::epub::IdentifierType::Url, def.homepage.as_str());
    if let Some(tl) = def.translator {
        book.add_translator(tl);
    }
    let mut has_failed = false;

    if let Some(cover) = def.cover_image {
        if let Err(e) = book.set_cover(Image::new(cover.as_str()), &cx).context("could not set cover") {
            error!("{e:?}");
            has_failed = true;
        }
    }

    let sections: HashMap<_, _> = def.sections.into_iter().map(|Section { title, start }| (start, title)).collect();
    let mut overrides = OverrideTracker::new(def.overrides);


    info!(target: "progress", "building chapters");
    for entry in def.content {
        match entry {
            def::UrlSelection::Range { start, end } => {
                if let Err(e) = fetch_range(&cx, &mut book, &rules, start, end, &sections, &mut overrides) {
                    error!("{e:?}");
                    has_failed = true;
                }
            },
            def::UrlSelection::Url(url) => {
                if let Err(e) = fetch_range(&cx, &mut book, &rules, url.clone(), url, &sections, &mut overrides) {
                    error!("{e:?}");
                    has_failed = true;
                }
            },
            def::UrlSelection::List(list) => {
                for url in list {
                    if let Err(e) = fetch_range(&cx, &mut book, &rules, url.clone(), url, &sections, &mut overrides) {
                        error!("{e:?}");
                        has_failed = true;
                    }
                }
            }
        }
    }

    if has_failed {
        bail!("aborting due to previous failures")
    }

    finish(book).context("failed writing epub")?;

    Ok(())
}

fn finish(book: EpubBuilder) -> anyhow::Result<()> {
    info!(target: "progress", "writing epub");
    let outpath = "output.epub";
    let mut outfile = std::fs::OpenOptions::new().write(true).read(false).truncate(true).create(true).open(outpath).context("could not open output epub")?;
    book.finish(&mut outfile).context("could not write to file")?;
    info!(target: "progress", "running epubcheck");
    if let Ok(res) = generate::epubcheck::epubcheck(outpath) {
        res.as_result(generate::epubcheck::Severity::Error)?;
        if let Err(e) = res.as_result(generate::epubcheck::Severity::Usage) {
            warn!("epubcheck warnings");
            warn!("{e}");
        }
    } else {
        warn!("could not run epubcheck")
    }
    Ok(())
}

fn fetch_range(cx: &FetchContext, book: &mut EpubBuilder<'_>, rules: &Rules, start: Url, end: Url, sections: &HashMap<Url, String>, track: &mut OverrideTracker) -> anyhow::Result<()> {
    ensure!(start.scheme() == end.scheme(), "start and end must be on the same scheme");
    ensure!(start.host_str() == end.host_str(), "start and end must be on the same host");
    let mut prev = None;
    let mut curr = start;
    loop {
        if let Some(section) = sections.get(&curr) {
            warn!("TODO: handle section {section}")
        }
        ensure!(curr.scheme() == "https" || curr.scheme() == "file", "url {curr} does not have expected scheme");
        let (ty, val) = cx.fetch(&curr).context("failed fetching")?;
        ensure!(ty == fetch::MediaType::Html, "{ty:?} is of wrong type");
        let html = std::str::from_utf8(&val).context("not valid utf-8")?;
        let html = Html::parse_document(&html);
        let html = Box::leak(Box::new(html));
        let overrides = track.with_url(&curr);
        let (ch, next) = rules.parse_with_overrides(html, &overrides, Some(cx)).context("failed to build chapter")?;
        let next = if let Some(next) = next {
            Some(Url::parse(next).context("invalid url")?)
        } else {
            None
        };
        book.add_chapter(ch);
        ensure!(prev.is_none() || prev != next, "url {} was repeated", prev.unwrap());
        if curr == end {
            break
        }

        let Some(next) = next else {
            warn!("expected more urls (up until {end}) but found no next after {curr}");
            break
        };
        prev = Some(curr);
        ensure!(next.host_str() == end.host_str(), "tried to go to different host for next url: {next}");
        curr = next
    }
    Ok(())
}

