use generate::lang::{Lang, StrLang, ALL_LANGS_STR};
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer,
};

struct StrLangV;

impl<'de> Visitor<'de> for StrLangV {
    type Value = StrLang;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("string or string in multiple languages")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(StrLang::from(v))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: de::MapAccess<'de>,
    {
        let Some((LangDe(lang), val)) = map.next_entry::<_, String>()? else {
            return Err(de::Error::missing_field("language"));
        };
        let mut ret = StrLang::new(lang, val);
        while let Some((LangDe(lang), val)) = map.next_entry::<_, String>()? {
            ret.try_set_alt(lang, val)
                .map_err(|_| de::Error::duplicate_field(lang.to_str()))?;
        }
        Ok(ret)
    }
}

struct LangV;

impl<'de> Visitor<'de> for LangV {
    type Value = Lang;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("language code")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        v.parse()
            .map_err(|_| de::Error::unknown_variant(v, &ALL_LANGS_STR))
    }
}

struct LangDe(Lang);
impl<'de> Deserialize<'de> for LangDe {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(LangV).map(LangDe)
    }
}

pub(super) fn lang_de<'de, D>(desel: D) -> Result<Lang, D::Error>
where
    D: Deserializer<'de>,
{
    desel.deserialize_any(LangV)
}

pub(super) fn strlang_de<'de, D>(desel: D) -> Result<StrLang, D::Error>
where
    D: Deserializer<'de>,
{
    desel.deserialize_any(StrLangV)
}
