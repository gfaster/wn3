use std::path::PathBuf;

use ahash::HashMap;
use anyhow::{bail, ensure, Context, Result};
use clap::{ArgAction, Parser, ValueEnum};
use common::Rules;
use def::BookDef;
use fetch::FetchContext;
use generate::{image::Image, EpubBuilder};
use log::{debug, error, info, warn};
use scraper::Html;
use url::Url;
use wn3::{def::Section, overrides::OverrideTracker, *};

mod logger;

const EXAMPLE_CFG: &str = include_str!("example.toml");

// NOTE 2024-08-05: I tried Jemalloc and it was slightly slower

#[derive(Parser, Debug)]
struct Args {
    /// input toml file
    #[arg(short_alias = 'i', alias = "spec", required_unless_present = "example")]
    spec: Option<PathBuf>,

    /// write example spec to stdout and then exit
    #[arg(long)]
    example: bool,

    #[arg(short, long, default_value = "output.epub")]
    output: PathBuf,

    #[arg(short, group = "verbosity", action = ArgAction::Count)]
    verbose: u8,

    /// don't print any warnings or errors
    #[arg(short, long, group = "verbosity")]
    quiet: bool,

    /// write each chapter as markdown to stdout
    #[arg(long)]
    dump: bool,

    /// don't make any web requests
    #[arg(long)]
    offline: bool,

    /// run `epubcheck` on the output
    #[arg(short, long)]
    check: bool,

    /// compression used for zip content
    #[arg(short = 'z', long, default_value_t = Compression::Deflate)]
    compression: Compression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Compression {
    Store,
    Deflate,
}

impl std::fmt::Display for Compression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Compression::Store => "store",
            Compression::Deflate => "deflate",
        }
        .fmt(f)
    }
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
    if args.quiet {
        log::set_max_level(log::LevelFilter::Off);
    }

    if args.example {
        println!("{EXAMPLE_CFG}");
        return Ok(());
    }

    build(&args)
}

fn build(args: &Args) -> Result<()> {
    let spec = args
        .spec
        .as_deref()
        .expect("spec is required if build is called");
    let f = std::fs::read_to_string(spec)
        .with_context(|| format!("failed to open spec {}", spec.display()))?;
    let def = {
        let mut def: BookDef = toml::from_str(&f).context("failed to parse spec")?;
        def.file = Some(spec.into());
        def
    };
    def.validate().context("spec invalid")?;
    let rules = Rules::new_il();
    let mut book = generate::EpubBuilder::new();
    let conn = rusqlite::Connection::open("cache.db")?;
    let client = ureq::AgentBuilder::new()
        .https_only(true)
        .user_agent("wn-scraper3/0.0.1 (github.com/gfaster)")
        .build();
    let fetch = FetchContext::new_cfg(conn, client, args.offline).unwrap();
    book.set_title(def.title)
        .add_author(def.author)
        .add_identifier(generate::epub::IdentifierType::Url, def.homepage.as_str());
    let compress = match args.compression {
        Compression::Store => generate::epub::Compression::Store,
        Compression::Deflate => generate::epub::Compression::Deflate,
    };
    book.set_compression(compress);

    if let Some(tl) = def.translator {
        book.add_translator(tl);
    }
    let mut has_failed = false;

    if let Some(cover) = def.cover_image {
        if let Err(e) = book
            .set_cover(Image::new(cover.as_str()), &fetch)
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

    let cx = ProgCx {
        fetch,
        rules,
        sections,
        args,
    };

    info!(target: "progress", "building chapters");
    for entry in def.content {
        match entry {
            def::UrlSelection::Range { start, end } => {
                if let Err(e) = fetch_range(&cx, &mut book, start, end, &mut overrides) {
                    error!("{e:?}");
                    has_failed = true;
                }
            }
            def::UrlSelection::Url(url) => {
                if let Err(e) = fetch_range(&cx, &mut book, url.clone(), url, &mut overrides) {
                    error!("{e:?}");
                    has_failed = true;
                }
            }
            def::UrlSelection::List(list) => {
                for url in list {
                    if let Err(e) = fetch_range(&cx, &mut book, url.clone(), url, &mut overrides) {
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
            error!("could not run epubcheck");
            bail!("could not run epubcheck")
        }
    } else {
        debug!("epubcheck is disabled")
    }
    Ok(())
}

struct ProgCx<'a> {
    fetch: FetchContext,
    rules: Rules,
    sections: HashMap<Url, String>,
    args: &'a Args,
}

fn fetch_range(
    cx: &ProgCx,
    book: &mut EpubBuilder<'_>,
    start: Url,
    end: Url,
    track: &mut OverrideTracker,
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
        if let Some(section) = cx.sections.get(&curr) {
            warn!("TODO: handle section {section}")
        }
        ensure!(
            curr.scheme() == "https" || curr.scheme() == "file",
            "url {curr} does not have expected scheme"
        );
        let (ty, val) = cx.fetch.fetch(&curr).context("failed fetching")?;
        ensure!(ty == fetch::MediaType::Html, "{ty:?} is of wrong type");
        let html = std::str::from_utf8(&val).context("not valid utf-8")?;
        let html = Html::parse_document(html);
        let html = Box::leak(Box::new(html));
        let overrides = track.with_url(&curr);
        let (ch, next) = cx
            .rules
            .parse_with_overrides(html, &overrides, Some(&cx.fetch))
            .context("failed to build chapter")?;
        let next = if let Some(next) = next {
            Some(Url::parse(next).context("invalid url")?)
        } else {
            None
        };
        if cx.args.dump {
            for ch in &ch {
                println!("{}\n", ch.md())
            }
        }
        book.extend_chapters(ch);
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

    #[test]
    fn example_is_valid_cfg() -> Result<()> {
        let def: BookDef = toml::from_str(EXAMPLE_CFG)?;
        def.validate().context("spec invalid")?;
        Ok(())
    }

    #[test]
    fn example_without_spec() {
        Args::try_parse_from("prog --example".split_whitespace()).unwrap();
        Args::try_parse_from("prog config.toml".split_whitespace()).unwrap();
    }

    #[ignore = "not working yet"]
    #[test]
    fn spec_flagged() -> Result<()> {
        Args::try_parse_from("prog -o output.epub --spec=config.toml".split_whitespace())?;
        Ok(())
    }
}
