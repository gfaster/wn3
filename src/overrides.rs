use std::{marker::PhantomData, rc::Rc};

use ahash::{HashMap, HashMapExt, HashSet};
use log::debug;
use url::Url;

use crate::def::{self, sed, UrlSelection};

pub struct OverrideSet<'a> {
    seds: Vec<Rc<[sed::Sed]>>,
    pub title: Option<String>,
    _ph: PhantomData<&'a OverrideTracker>,
}

impl OverrideSet<'_> {
    pub const fn empty() -> Self {
        OverrideSet {
            seds: Vec::new(),
            title: None,
            _ph: PhantomData,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.seds.is_empty() && self.title.is_none()
    }

    pub fn replacers(&self) -> impl Iterator<Item = &sed::Sed> {
        self.seds.iter().flat_map(|x| x.as_ref())
    }
}

impl std::fmt::Debug for OverrideSet<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("OverrideSet");
        // unnecessary allocs
        let v: Vec<_> = self.replacers().map(|r| r.to_string()).collect();
        if !v.is_empty() {
            d.field("seds", &v);
        }

        if let Some(title) = &self.title {
            d.field("title", &title);
        }

        d.finish()
    }
}

struct OverrideChoice {
    urls: UrlSelection,
    title: Option<String>,
    subs: Rc<[sed::Sed]>,
}

pub struct OverrideTracker {
    /// active until key hit (deactivated after)
    active: HashMap<Box<str>, Vec<OverrideChoice>>,

    /// activated when url key
    unactivated: HashMap<Box<str>, Vec<OverrideChoice>>,
}

impl OverrideTracker {
    pub fn new(overrides: Vec<def::OverrideChoice>) -> Self {
        let mut unactivated: HashMap<Box<str>, Vec<_>> = HashMap::new();
        for entry in overrides {
            let subs: Rc<[_]> = entry.subs.into();
            match entry.urls {
                UrlSelection::Range { start, end } => {
                    unactivated
                        .entry(start.as_str().into())
                        .or_default()
                        .push(OverrideChoice {
                            urls: UrlSelection::Range { start, end },
                            title: None,
                            subs,
                        });
                }
                UrlSelection::Url(url) => {
                    unactivated
                        .entry(url.as_str().into())
                        .or_default()
                        .push(OverrideChoice {
                            urls: UrlSelection::Url(url),
                            title: entry.title.clone(),
                            subs,
                        });
                }
                UrlSelection::List(urls) => {
                    let urls = HashSet::from_iter(urls);
                    for url in urls {
                        unactivated
                            .entry(url.as_str().into())
                            .or_default()
                            .push(OverrideChoice {
                                urls: UrlSelection::Url(url),
                                // I don't expect this to be used
                                title: entry.title.clone(),
                                subs: subs.clone(),
                            });
                    }
                }
            }
        }
        OverrideTracker {
            active: HashMap::new(),
            unactivated,
        }
    }

    pub fn with_url<'a>(&'a mut self, url: &Url) -> OverrideSet<'a> {
        // PERF: unnecessary clones here
        // PERF: unnecessary remove and then add for single
        if let Some(new_v) = self.unactivated.remove(url.as_str()) {
            for entry in new_v {
                let k: Box<str> = match &entry.urls {
                    UrlSelection::Url(url) => url.as_str().into(),
                    UrlSelection::Range { end, .. } => end.as_str().into(),
                    UrlSelection::List(_) => unreachable!(),
                };
                self.active.entry(k).or_default().push(entry);
            }
        }
        let mut ret = OverrideSet {
            seds: Vec::new(),
            title: None,
            _ph: PhantomData,
        };
        if let Some(ending) = self.active.remove(url.as_str()) {
            for entry in ending {
                if entry.title.is_some() {
                    ret.title = entry.title.clone();
                }
                ret.seds.push(entry.subs)
            }
        }
        for active in self.active.values() {
            for entry in active {
                match &entry.urls {
                    UrlSelection::Url(_) => {
                        unreachable!("single urls were removed prior")
                    }
                    UrlSelection::Range { start: _, end: _ } => {
                        ret.seds.push(entry.subs.clone());
                    }
                    UrlSelection::List(_) => unreachable!(),
                }
            }
        }
        if !ret.is_empty() {
            debug!("overrides for {url}: {ret:#?}");
        }
        ret
    }
}
