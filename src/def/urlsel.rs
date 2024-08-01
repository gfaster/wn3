use serde::{
    de::{self, Visitor},
    Deserialize,
};
use url::Url;

#[derive(Debug, PartialEq, Eq)]
pub enum UrlSelection {
    Range { start: Url, end: Url },
    Url(Url),
    List(Vec<Url>),
}

impl UrlSelection {
    pub fn as_slice(&self) -> Option<&[Url]> {
        match self {
            UrlSelection::Range { .. } => None,
            UrlSelection::Url(url) => Some(std::slice::from_ref(url)),
            UrlSelection::List(l) => Some(l),
        }
    }
}

struct UrlSelVisitor;

impl<'de> Deserialize<'de> for UrlSelection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_any(UrlSelVisitor)
    }
}

impl<'de> Visitor<'de> for UrlSelVisitor {
    type Value = UrlSelection;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a url, list of urls, or range of urls with start and end")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        // I think there is a better way of doing this?
        Url::parse(v)
            .map(|url| UrlSelection::Url(url))
            .map_err(|_| E::invalid_value(de::Unexpected::Str(v), &"a valid url"))
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        let mut v = Vec::new();
        if let Some(sz) = seq.size_hint() {
            v.reserve(sz)
        }
        while let Some(url) = seq.next_element::<Url>()? {
            v.push(url);
        }
        Ok(UrlSelection::List(v))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: de::MapAccess<'de>,
    {
        // https://serde.rs/deserialize-struct.html
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum Field {
            Start,
            End,
            Url,
            Urls,
        }
        let mut start = None;
        let mut end = None;
        let mut url = None;
        let mut urls = None;
        while let Some(key) = map.next_key::<Field>()? {
            match key {
                Field::Start => {
                    if start.is_some() {
                        return Err(de::Error::duplicate_field("start"));
                    }
                    if urls.is_some() || url.is_some() {
                        return Err(de::Error::duplicate_field("url range"));
                    }
                    start = Some(map.next_value()?);
                }
                Field::End => {
                    if end.is_some() {
                        return Err(de::Error::duplicate_field("end"));
                    }
                    if urls.is_some() || url.is_some() {
                        return Err(de::Error::duplicate_field("url range"));
                    }
                    end = Some(map.next_value()?);
                }
                Field::Url => {
                    if url.is_some() {
                        return Err(de::Error::duplicate_field("url"));
                    }
                    if urls.is_some() || start.is_some() || end.is_some() {
                        return Err(de::Error::duplicate_field("url range"));
                    }
                    url = Some(map.next_value()?);
                }
                Field::Urls => {
                    if urls.is_some() {
                        return Err(de::Error::duplicate_field("urls"));
                    }
                    if url.is_some() || start.is_some() || end.is_some() {
                        return Err(de::Error::duplicate_field("url range"));
                    }
                    urls = Some(map.next_value()?);
                }
            }
        }
        match (start, end) {
            (Some(start), Some(end)) => return Ok(UrlSelection::Range { start, end }),
            (None, None) => (),
            (None, Some(_)) => return Err(de::Error::missing_field("start")),
            (Some(_), None) => return Err(de::Error::missing_field("end")),
        }
        if let Some(url) = url {
            return Ok(UrlSelection::Url(url));
        }
        if let Some(urls) = urls {
            return Ok(UrlSelection::List(urls));
        }

        Err(de::Error::invalid_value(
            de::Unexpected::StructVariant,
            &"either single url, range start..=end, or list",
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::ops::RangeInclusive;

    use super::*;

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    #[serde(deny_unknown_fields)]
    struct Test {
        content: Vec<UrlSelection>,
    }

    fn url(s: &str) -> UrlSelection {
        UrlSelection::Url(Url::parse(s).unwrap())
    }

    fn urls(s: &[&str]) -> UrlSelection {
        let v = s.iter().map(|s| Url::parse(s).unwrap()).collect();
        UrlSelection::List(v)
    }

    fn urlr(s: RangeInclusive<&str>) -> UrlSelection {
        UrlSelection::Range {
            start: Url::parse(s.start()).unwrap(),
            end: Url::parse(s.end()).unwrap(),
        }
    }

    #[test]
    fn de_tables_one() {
        let s = r#"
        [[content]]
        url = "https://example.com/0"
        [[content]]
        url = "https://example.com/1"
        "#;
        let actual: Test = toml::from_str(s).unwrap();
        let expected = Test {
            content: vec![url("https://example.com/0"), url("https://example.com/1")],
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn de_tables_list_table() {
        let s = r#"
        [[content]]
        urls = ["https://example.com/0", "https://example.com/1"]

        [[content]]
        url = "https://example.com/2"
        "#;
        let actual: Test = toml::from_str(s).unwrap();
        let expected = Test {
            content: vec![
                urls(&["https://example.com/0", "https://example.com/1"]),
                url("https://example.com/2"),
            ],
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn de_tables_list_ooo_fields() {
        let s = r#"
        [[content]]
        end = "https://example.com/1"
        start = "https://example.com/0"
        "#;
        let actual: Test = toml::from_str(s).unwrap();
        let expected = Test {
            content: vec![urlr("https://example.com/0"..="https://example.com/1")],
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn de_tables_list_pure() {
        let s = r#"
        content = ["https://example.com/0", "https://example.com/1"]
        "#;
        let actual: Test = toml::from_str(s).unwrap();
        let expected = Test {
            content: vec![url("https://example.com/0"), url("https://example.com/1")],
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn de_tables_list_hybrid() {
        let s = r#"
        content = [
            "https://example.com/0",
            "https://example.com/1",
            {start = "https://example.com/2", end = "https://example.com/4" },
            {url = "https://example.com/5" }
        ]
        "#;
        let actual: Test = toml::from_str(s).unwrap();
        let expected = Test {
            content: vec![
                url("https://example.com/0"),
                url("https://example.com/1"),
                urlr("https://example.com/2"..="https://example.com/4"),
                url("https://example.com/5"),
            ],
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn de_tables_list_table_2() {
        let s = r#"
        [[content]]
        urls = ["https://example.com/0", "https://example.com/1"]

        [[content]]
        start = "https://example.com/2"
        end = "https://example.com/4"

        [[content]]
        url = "https://example.com/5"
        "#;
        let actual: Test = toml::from_str(s).unwrap();
        let expected = Test {
            content: vec![
                urls(&["https://example.com/0", "https://example.com/1"]),
                urlr("https://example.com/2"..="https://example.com/4"),
                url("https://example.com/5"),
            ],
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn de_fail_dup() {
        let s = r#"
        [[content]]
        urls = ["https://example.com/0", "https://example.com/1"]
        urls = ["https://example.com/2"]
        "#;
        toml::from_str::<Test>(s).unwrap_err();

        let s = r#"
        [[content]]
        start = "https://example.com/0"
        end = "https://example.com/1"
        start = "https://example.com/2"
        "#;
        toml::from_str::<Test>(s).unwrap_err();

        let s = r#"
        [[content]]
        end = "https://example.com/2"
        start = "https://example.com/0"
        end = "https://example.com/1"
        "#;
        toml::from_str::<Test>(s).unwrap_err();
    }

    #[test]
    fn de_fail_conflict() {
        let s = r#"
        [[content]]
        urls = ["https://example.com/0", "https://example.com/1"]
        url = "https://example.com/2"
        "#;
        toml::from_str::<Test>(s).unwrap_err();

        let s = r#"
        [[content]]
        start = "https://example.com/0"
        url = "https://example.com/2"
        "#;
        toml::from_str::<Test>(s).unwrap_err();

        let s = r#"
        [[content]]
        end = "https://example.com/2"
        url = "https://example.com/2"
        "#;
        toml::from_str::<Test>(s).unwrap_err();

        let s = r#"
        [[content]]
        urls = ["https://example.com/0", "https://example.com/1"]
        start = "https://example.com/2"
        "#;
        toml::from_str::<Test>(s).unwrap_err();

        let s = r#"
        [[content]]
        urls = ["https://example.com/0", "https://example.com/1"]
        end = "https://example.com/2"
        "#;
        toml::from_str::<Test>(s).unwrap_err();

        let s = r#"
        [[content]]
        end = "https://example.com/2"
        urls = ["https://example.com/0", "https://example.com/1"]
        "#;
        toml::from_str::<Test>(s).unwrap_err();
    }
}
