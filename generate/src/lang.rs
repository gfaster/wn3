// TODO: all languages
// https://www.iana.org/assignments/language-subtag-registry/language-subtag-registry

use std::{fmt, str::FromStr};

/// I'm sorry, but this just makes things a bunch easier.
pub const DEFAULT_LANG: Lang = Lang::En;

pub const ALL_LANGS: [Lang; 4] = [Lang::En, Lang::De, Lang::Zh, Lang::Ja];
pub const ALL_LANGS_STR: [&'static str; 4] = ["en", "de", "zh", "ja"];

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Lang {
    En,
    De,
    Zh,
    Ja,
}

impl Default for Lang {
    fn default() -> Self {
        DEFAULT_LANG
    }
}

impl Lang {
    pub fn new(s: &str) -> Option<Self> {
        let l = match s {
            "en" => Lang::En,
            "de" => Lang::De,
            "zh" => Lang::Zh,
            "ja" => Lang::Ja,
            _ => return None,
        };
        Some(l)
    }

    pub const fn as_str(self) -> &'static str {
        self.to_str()
    }

    pub const fn to_str(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::De => "de",
            Lang::Zh => "zh",
            Lang::Ja => "ja",
        }
    }
}

impl fmt::Debug for Lang {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.to_str().fmt(f)
    }
}

impl fmt::Display for Lang {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.to_str().fmt(f)
    }
}

impl FromStr for Lang {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Lang::new(s).ok_or("invalid or unimplemented language")
    }
}

/// a string with possible alternate scripts
///
/// see: <https://www.w3.org/TR/epub/#sec-alternate-script>
// FIXME: the eq implementation is wrong
#[derive(Debug, PartialEq, Eq)]
pub struct StrLang(StrLangInner);

#[derive(Debug, PartialEq, Eq)]
enum StrLangInner {
    // have funny repr for iter implementation
    Single((Lang, Box<str>)),
    Many(Vec<(Lang, Box<str>)>),
}

impl StrLang {
    pub fn new(lang: Lang, s: impl Into<Box<str>>) -> Self {
        StrLang(StrLangInner::Single((lang, s.into())))
    }

    pub fn no_alts(&self) -> bool {
        matches!(self.0, StrLangInner::Single(_))
    }

    /// sets the primary language, returns error if there are mutliple languages
    pub fn set_primary_lang(&mut self, new_lang: Lang) -> Result<(), &'static str> {
        let StrLangInner::Single((lang, _)) = &mut self.0 else {
            return Err("cannot set primary language ");
        };
        *lang = new_lang;
        Ok(())
    }

    /// sets the alternate script for lang, overwritting the existing if it exists
    pub fn set_alt(&mut self, lang: Lang, s: impl Into<Box<str>>) {
        match &mut self.0 {
            StrLangInner::Single((old_lang, old_s)) if *old_lang == lang => *old_s = s.into(),
            StrLangInner::Single((first_lang, first_s)) => {
                let first_s = std::mem::take(first_s);
                let first = (*first_lang, first_s);
                self.0 = StrLangInner::Many(vec![first, (lang, s.into())]);
            }
            StrLangInner::Many(v) => {
                if let Some(old) = v
                    .iter_mut()
                    .find_map(|(l, old)| (*l == lang).then_some(old))
                {
                    *old = s.into();
                } else {
                    v.push((lang, s.into()))
                }
            }
        }
    }

    /// sets the alternate script for lang, returning Err if it exists
    pub fn try_set_alt(&mut self, lang: Lang, s: impl Into<Box<str>>) -> Result<(), ()> {
        match &mut self.0 {
            StrLangInner::Single((old_lang, _)) if *old_lang == lang => return Err(()),
            StrLangInner::Single((first_lang, first_s)) => {
                let first_s = std::mem::take(first_s);
                let first = (*first_lang, first_s);
                self.0 = StrLangInner::Many(vec![first, (lang, s.into())]);
            }
            StrLangInner::Many(v) => {
                if v.iter_mut().find(|(l, _)| *l == lang).is_some() {
                    return Err(());
                } else {
                    v.push((lang, s.into()))
                }
            }
        }
        Ok(())
    }

    pub fn for_lang(&self, lang: Lang) -> Option<&str> {
        match &self.0 {
            StrLangInner::Single((l, s)) if *l == lang => Some(&**s),
            StrLangInner::Single(_) => None,
            StrLangInner::Many(v) => v.iter().find(|(l, _)| *l == lang).map(|(_, s)| &**s),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (Lang, &str)> {
        match &self.0 {
            StrLangInner::Many(v) => v.iter(),
            StrLangInner::Single(l) => std::slice::from_ref(l).iter(),
        }
        .map(|(l, s)| (*l, &**s))
    }
}

impl From<&str> for StrLang {
    fn from(value: &str) -> Self {
        StrLang::new(Lang::default(), value)
    }
}

impl From<String> for StrLang {
    fn from(value: String) -> Self {
        StrLang::new(Lang::default(), value)
    }
}

impl From<Box<str>> for StrLang {
    fn from(value: Box<str>) -> Self {
        StrLang::new(Lang::default(), value)
    }
}
