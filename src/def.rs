use serde::Deserialize;

// TODO: write custom deserialize impls

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BookDef {
    pub title: String,
    pub url: String,
    pub content: Vec<ContentEntry>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
#[serde(deny_unknown_fields)]
#[serde(rename_all_fields = "kebab-case")]
pub enum ContentEntry {
    UrlRange {
        ruleset_override: Option<String>,
        #[serde(default)]
        exclude_urls: Vec<String>,
        start: String,
        end: String,
    },
    Url {
        ruleset_override: Option<String>,
        title_override: Option<String>,
        url: String,
    },
    Section {
        section_title: String,
    }
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
    use super::*;


    #[test]
    fn it_works_chapters() {
        let def = r#"
        title = "Example"
        url = "https://example.com"

        [[content]]
        start = "https://example.com/1"
        end = "https://example.com/4"

        [[content]]
        url = "https://example.com/5"

        [[content]]
        ruleset-override = "weird"
        url = "https://example.com/6"
        "#;
        let actual: BookDef = toml::from_str(def).map_err(|e| eprintln!("{e}")).unwrap();
        let expected = BookDef {
            title: "Example".into(),
            url: "https://example.com".into(),
            content: vec![
                ContentEntry::UrlRange {
                    ruleset_override: None,
                    exclude_urls: vec![],
                    start: "https://example.com/1".into(),
                    end: "https://example.com/4".into(),
                },
                ContentEntry::Url {
                    title_override: None,
                    ruleset_override: None,
                    url: "https://example.com/5".into() 
                },
                ContentEntry::Url {
                    title_override: None,
                    ruleset_override: Some("weird".into()),
                    url: "https://example.com/6".into() 
                },
            ],
        };
        expected.validate().unwrap();
        actual.validate().unwrap();
        assert_eq!(actual, expected)
    }

    #[test]
    fn it_works_sections() {
        let def = r#"
        title = "Example"
        url = "https://example.com"

        [[content]]
        title-override = "prologue"
        url = "https://example.com/0"

        [[content]]
        section-title = "section 1"

        [[content]]
        exclude-urls = ["https://example.com/2"]
        start = "https://example.com/1"
        end = "https://example.com/4"

        [[content]]
        section-title = "section 2"

        [[content]]
        url = "https://example.com/5"

        [[content]]
        ruleset-override = "weird"
        url = "https://example.com/6"
        "#;
        let actual: BookDef = toml::from_str(def).map_err(|e| eprintln!("{e}")).unwrap();
        let expected = BookDef {
            title: "Example".into(),
            url: "https://example.com".into(),
            content: vec![
                ContentEntry::Url {
                    title_override: Some("prologue".into()),
                    ruleset_override: None,
                    url: "https://example.com/0".into() 
                },
                ContentEntry::Section { section_title: "section 1".into() },
                ContentEntry::UrlRange {
                    exclude_urls: vec!["https://example.com/2".into()],
                    ruleset_override: None,
                    start: "https://example.com/1".into(),
                    end: "https://example.com/4".into(),
                },
                ContentEntry::Section { section_title: "section 2".into() },
                ContentEntry::Url {
                    title_override: None,
                    ruleset_override: None,
                    url: "https://example.com/5".into() 
                },
                ContentEntry::Url {
                    title_override: None,
                    ruleset_override: Some("weird".into()),
                    url: "https://example.com/6".into() 
                },
            ],
        };
        expected.validate().unwrap();
        actual.validate().unwrap();
        assert_eq!(actual, expected)
    }
}
