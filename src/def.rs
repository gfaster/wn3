use std::path::PathBuf;

use log::warn;
use serde::Deserialize;
use url::Url;

pub mod sed;
mod urlsel;
pub use urlsel::UrlSelection;

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct BookDef {
    #[serde(skip)]
    pub file: Option<PathBuf>,

    pub title: String,
    pub author: String,
    pub subtitle: Option<String>,
    pub homepage: Url,
    pub cover_image: Option<Url>,
    pub translator: Option<String>,
    pub content: Vec<UrlSelection>,
    #[serde(default)]
    pub overrides: Vec<OverrideChoice>,
    #[serde(default)]
    pub sections: Vec<Section>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
#[serde(deny_unknown_fields)]
pub struct Section {
    pub title: String,
    pub start: Url,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
#[serde(deny_unknown_fields)]
pub struct OverrideChoice {
    #[serde(alias = "url")]
    pub urls: UrlSelection,
    pub title: Option<String>,
    #[serde(default, alias = "rules")]
    pub subs: Vec<sed::Sed>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct BookDefValidationError {}

impl std::fmt::Display for BookDefValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

impl std::error::Error for BookDefValidationError {}

impl BookDef {
    pub fn validate(&self) -> Result<(), BookDefValidationError> {
        fn log_if_todo_opt(s: &Option<String>, field: &str) -> bool {
            s.as_ref().map(|s| log_if_todo(s, field)).unwrap_or(false)
        }
        fn log_if_todo(s: &str, field: &str) -> bool {
            if s.eq_ignore_ascii_case("todo") {
                warn!("config field `{field}` is marked as TODO");
                return true;
            }
            false
        }
        let mut w = false;
        w |= log_if_todo(&self.title, "title");
        w |= log_if_todo_opt(&self.subtitle, "subtitle");
        w |= log_if_todo(&self.author, "author");
        w |= log_if_todo_opt(&self.translator, "translator");
        if w {
            match &self.file {
                Some(file) => warn!("config file `{}` has warnings", file.display()),
                None => warn!("book definition has warnings"),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {}
