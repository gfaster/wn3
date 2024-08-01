use std::path::PathBuf;

use ahash::HashMap;
use anyhow::{bail, ensure, Context, Result};
use clap::{ArgAction, Parser};
use common::Rules;
use def::BookDef;
use fetch::FetchContext;
use generate::{image::Image, EpubBuilder};
use log::{debug, error, info, warn};
use scraper::Html;
use url::Url;
use wn3::{def::Section, overrides::OverrideTracker, *};

mod logger;

#[derive(Parser, Debug)]
struct Args {
    /// input toml file
    #[arg(short_alias = 'i', alias = "spec", required = true)]
    spec: PathBuf,

    #[arg(short, long, default_value = "output.epub")]
    output: PathBuf,

    #[arg(short, group = "verbosity", action = ArgAction::Count)]
    verbose: u8,

    #[arg(short, long, group = "verbosity")]
    quiet: bool,

    #[arg(long)]
    dump: bool,

    #[arg(long)]
    offline: bool,

    #[arg(short, long)]
    check: bool,
}

fn main() -> Result<()> {
    logger::init().unwrap();
    let args = Args::parse();
    match args.verbose {
        0 => log::set_max_level(log::LevelFilter::Info),
        1 => log::set_max_level(log::LevelFilter::Debug),
        2 => log::set_max_level(log::LevelFilter::Trace),
        _ => bail!("maximum verbosity is 2 (-vv)"),
    }

    build(&args)
}

fn build(args: &Args) -> Result<()> {
    let f = std::fs::read_to_string(&args.spec)
        .with_context(|| format!("failed to open spec {}", args.spec.display()))?;
    let def: BookDef = toml::from_str(&f).context("failed to parse spec")?;
    def.validate().context("spec invalid")?;
    let rules = Rules::new_il();
    let mut book = generate::EpubBuilder::new();
    let conn = rusqlite::Connection::open("cache.db")?;
    let client = ureq::AgentBuilder::new()
        .https_only(true)
        .user_agent("wn-scraper3/0.0.1 (github.com/gfaster)")
        .build();
    let cx = FetchContext::new_cfg(conn, client, args.offline).unwrap();
    book.set_title(def.title);
    book.add_identifier(generate::epub::IdentifierType::Url, def.homepage.as_str());
    if let Some(tl) = def.translator {
        book.add_translator(tl);
    }
    let mut has_failed = false;

    if let Some(cover) = def.cover_image {
        if let Err(e) = book
            .set_cover(Image::new(cover.as_str()), &cx)
            .context("could not set cover")
        {
            error!("{e:?}");
            has_failed = true;
        }
    }

    let sections: HashMap<_, _> = def
        .sections
        .into_iter()
        .map(|Section { title, start }| (start, title))
        .collect();
    let mut overrides = OverrideTracker::new(def.overrides);

    info!(target: "progress", "building chapters");
    for entry in def.content {
        match entry {
            def::UrlSelection::Range { start, end } => {
                if let Err(e) = fetch_range(
                    &cx,
                    &mut book,
                    &rules,
                    start,
                    end,
                    &sections,
                    &mut overrides,
                    args,
                ) {
                    error!("{e:?}");
                    has_failed = true;
                }
            }
            def::UrlSelection::Url(url) => {
                if let Err(e) = fetch_range(
                    &cx,
                    &mut book,
                    &rules,
                    url.clone(),
                    url,
                    &sections,
                    &mut overrides,
                    args,
                ) {
                    error!("{e:?}");
                    has_failed = true;
                }
            }
            def::UrlSelection::List(list) => {
                for url in list {
                    if let Err(e) = fetch_range(
                        &cx,
                        &mut book,
                        &rules,
                        url.clone(),
                        url,
                        &sections,
                        &mut overrides,
                        args,
                    ) {
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

    finish(book, args).context("failed writing epub")?;

    Ok(())
}

fn finish(book: EpubBuilder, args: &Args) -> anyhow::Result<()> {
    info!(target: "progress", "writing to {}", args.output.display());
    let mut outfile = std::fs::OpenOptions::new()
        .write(true)
        .read(false)
        .truncate(true)
        .create(true)
        .open(&args.output)
        .with_context(|| format!("could not open {}", args.output.display()))?;
    book.finish(&mut outfile)
        .context("could not write to file")?;
    if args.check {
        info!(target: "progress", "running epubcheck");
        if let Ok(res) = generate::epubcheck::epubcheck(&args.output) {
            res.as_result(generate::epubcheck::Severity::Error)?;
            if let Err(e) = res.as_result(generate::epubcheck::Severity::Usage) {
                warn!("epubcheck warnings");
                warn!("{e}");
            }
        } else {
            warn!("could not run epubcheck")
        }
    } else {
        debug!("epubcheck is disabled")
    }
    Ok(())
}

fn fetch_range(
    cx: &FetchContext,
    book: &mut EpubBuilder<'_>,
    rules: &Rules,
    start: Url,
    end: Url,
    sections: &HashMap<Url, String>,
    track: &mut OverrideTracker,
    args: &Args,
) -> anyhow::Result<()> {
    ensure!(
        start.scheme() == end.scheme(),
        "start and end must be on the same scheme"
    );
    ensure!(
        start.host_str() == end.host_str(),
        "start and end must be on the same host"
    );
    let mut prev = None;
    let mut curr = start;
    loop {
        if let Some(section) = sections.get(&curr) {
            warn!("TODO: handle section {section}")
        }
        ensure!(
            curr.scheme() == "https" || curr.scheme() == "file",
            "url {curr} does not have expected scheme"
        );
        let (ty, val) = cx.fetch(&curr).context("failed fetching")?;
        ensure!(ty == fetch::MediaType::Html, "{ty:?} is of wrong type");
        let html = std::str::from_utf8(&val).context("not valid utf-8")?;
        let html = Html::parse_document(&html);
        let html = Box::leak(Box::new(html));
        let overrides = track.with_url(&curr);
        let (ch, next) = rules
            .parse_with_overrides(html, &overrides, Some(cx))
            .context("failed to build chapter")?;
        let next = if let Some(next) = next {
            Some(Url::parse(next).context("invalid url")?)
        } else {
            None
        };
        if args.dump {
            println!("{ch:#}")
        }
        book.add_chapter(ch);
        ensure!(
            prev.is_none() || prev != next,
            "url {} was repeated",
            prev.unwrap()
        );
        if curr == end {
            break;
        }

        let Some(next) = next else {
            warn!("expected more urls (up until {end}) but found no next after {curr}");
            break;
        };
        prev = Some(curr);
        ensure!(
            next.host_str() == end.host_str(),
            "tried to go to different host for next url: {next}"
        );
        curr = next
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn args_valid() {
        Args::command().debug_assert();
    }
}
