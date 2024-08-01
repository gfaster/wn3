use serde::Deserialize;
use url::Url;

pub mod sed;

// TODO: write custom deserialize impls

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct BookDef {
    pub title: String,
    pub subtitle: Option<String>,
    pub homepage: Url,
    pub cover_image: Option<Url>,
    pub author: Option<String>,
    pub translator: Option<String>,
    pub content: Vec<UrlSelection>,
    #[serde(default)]
    pub overrides: Vec<OverrideChoice>,
    #[serde(default)]
    pub sections: Vec<Section>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
#[serde(deny_unknown_fields)]
#[serde(rename_all_fields = "kebab-case")]
#[serde(rename_all = "kebab-case")]
pub enum UrlSelection {
    Range { start: Url, end: Url },
    Url(Url),
    List(Vec<Url>),
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
    #[serde(default)]
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
        Ok(())
    }
}

#[cfg(test)]
mod tests {}
