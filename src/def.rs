use serde::Deserialize;
use url::Url;

pub mod sed;

// TODO: write custom deserialize impls

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BookDef {
    pub title: String,
    pub homepage: Url,
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
pub enum UrlSelection {
    Range {
        start: Url,
        end: Url,
    },
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
    pub subs: Vec<sed::SedLite>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct BookDefValidationError {
}

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
mod tests {
    // use super::*;
    //
    // fn url(s: &str) -> Url {
    //     Url::parse(s).unwrap()
    // }
    //
    // #[test]
    // fn it_works_chapters() {
    //     let def = r#"
    //     title = "Example"
    //     url = "https://example.com"
    //
    //     [[content]]
    //     start = "https://example.com/1"
    //     end = "https://example.com/4"
    //
    //     [[content]]
    //     url = "https://example.com/5"
    //
    //     [[content]]
    //     ruleset-override = "weird"
    //     url = "https://example.com/6"
    //     "#;
    //     let actual: BookDef = toml::from_str(def).map_err(|e| eprintln!("{e}")).unwrap();
    //     let expected = BookDef {
    //     };
    //     expected.validate().unwrap();
    //     actual.validate().unwrap();
    //     assert_eq!(actual, expected)
    // }
    //
    // #[test]
    // fn it_works_sections() {
    //     let def = r#"
    //     title = "Example"
    //     url = "https://example.com"
    //
    //     [[content]]
    //     title-override = "prologue"
    //     url = "https://example.com/0"
    //
    //     [[content]]
    //     section-title = "section 1"
    //
    //     [[content]]
    //     exclude-urls = ["https://example.com/2"]
    //     start = "https://example.com/1"
    //     end = "https://example.com/4"
    //
    //     [[content]]
    //     section-title = "section 2"
    //
    //     [[content]]
    //     url = "https://example.com/5"
    //
    //     [[content]]
    //     ruleset-override = "weird"
    //     url = "https://example.com/6"
    //     "#;
    //     let actual: BookDef = toml::from_str(def).map_err(|e| eprintln!("{e}")).unwrap();
    //     let expected = BookDef {
    //         title: "Example".into(),
    //         homepage: "https://example.com".into(),
    //         content: vec![
    //         ],
    //     };
    //     expected.validate().unwrap();
    //     actual.validate().unwrap();
    //     assert_eq!(actual, expected)
    // }
}
