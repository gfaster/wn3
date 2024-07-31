use std::borrow::Cow;

use regex_lite::{NoExpand, Regex};
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
#[serde(try_from = "String")]
pub struct SedLite {
    find: Regex,
    replace: Box<str>,
}

impl From<SedLite> for Cow<'_, SedLite> {
    fn from(value: SedLite) -> Self {
        Cow::Owned(value)
    }
}

impl<'a> From<&'a SedLite> for Cow<'a, SedLite> {
    fn from(value: &'a SedLite) -> Self {
        Cow::Borrowed(value)
    }
}

impl PartialEq for SedLite {
    fn eq(&self, other: &Self) -> bool {
        self.find.as_str() == other.find.as_str() && self.replace == other.replace
    }
}

impl Eq for SedLite {}

impl SedLite {
    /// this will be quite hot, so I'm going to try to make it efficient
    pub fn apply<'a>(&self, s: impl Into<Cow<'a, str>>) -> Cow<'a, str> {
        let s: Cow<str> = s.into();
        let res = self.find.replace_all(&s, NoExpand(&self.replace));
        match res {
            Cow::Borrowed(_) => s,
            Cow::Owned(res) => Cow::Owned(res),
        }
    }
}

pub struct SedLiteError(SedLiteErrorInner);

enum SedLiteErrorInner {
    RegexParseError(regex_lite::Error),
    MissingLeadingS,
    IncorrectSlash,
}

impl std::fmt::Display for SedLiteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            SedLiteErrorInner::RegexParseError(e) => e.fmt(f),
            SedLiteErrorInner::MissingLeadingS => "Missing leading 's', search Vim help :s".fmt(f),
            SedLiteErrorInner::IncorrectSlash => {
                "Missing or incorrect '/', note escaping not yet supported".fmt(f)
            }
        }
    }
}

impl std::fmt::Debug for SedLiteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

impl TryFrom<&str> for SedLite {
    type Error = SedLiteError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let Some(value) = value.strip_prefix("s") else {
            return Err(SedLiteError(SedLiteErrorInner::MissingLeadingS));
        };
        let Some(value) = value.strip_prefix("/") else {
            return Err(SedLiteError(SedLiteErrorInner::IncorrectSlash));
        };
        let Some(value) = value.strip_suffix("/") else {
            return Err(SedLiteError(SedLiteErrorInner::IncorrectSlash));
        };
        let Some((find, replace)) = value.split_once('/') else {
            return Err(SedLiteError(SedLiteErrorInner::IncorrectSlash));
        };
        if replace.contains('/') {
            return Err(SedLiteError(SedLiteErrorInner::IncorrectSlash));
        }
        let find =
            Regex::new(find).map_err(|e| SedLiteError(SedLiteErrorInner::RegexParseError(e)))?;
        Ok(SedLite {
            find,
            replace: replace.into(),
        })
    }
}

impl TryFrom<String> for SedLite {
    type Error = SedLiteError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.as_str().try_into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let sed: SedLite = "s/abc/xyz/".try_into().unwrap();
        assert_eq!(sed.apply("abc"), "xyz");
        assert_eq!(sed.apply("aabcc"), "axyzc");
        let sed: SedLite = "s/a+bc+/xyz/".try_into().unwrap();
        assert_eq!(sed.apply("abc"), "xyz");
        assert_eq!(sed.apply("aabcc"), "xyz");
        assert_eq!(sed.apply("aab"), "aab");
    }

    #[test]
    fn invalid_fails() {
        assert!(SedLite::try_from("/abc/xyz/").is_err());
        assert!(SedLite::try_from("sabc/xyz/").is_err());
        assert!(SedLite::try_from("s/xyz/").is_err());
        assert!(SedLite::try_from("s/").is_err());
        assert!(SedLite::try_from("s/abc/xyz//").is_err());
    }
}
