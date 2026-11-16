//! ADR-008: a value of a state file read loosely. A state file may be
//! written by hand or by a newer Underline; a field whose value has an
//! unexpected form (a list where a word was, an integer beyond 64 bits, a
//! tag) must not lose the rest of the file. [`Loose`] takes any YAML value,
//! and the field's own reader decides what it makes of it.

use std::fmt;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

/// Any value, as much of it as a field of the state can use.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Loose {
    /// A string.
    Text(String),
    /// A number of any size, as a float (`inf` beyond its range).
    Number(f64),
    /// A list.
    List(Vec<Loose>),
    /// Anything else: null, a bool, a map, bytes, a tagged value.
    Other,
}

impl Loose {
    /// The value of a field read by `read`: [`Loose`] made into what the
    /// field takes. For `#[serde(deserialize_with = "…")]`.
    pub(crate) fn read<'de, D, T>(
        deserializer: D,
        read: impl FnOnce(Loose) -> T,
    ) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        Loose::deserialize(deserializer).map(read)
    }
}

impl<'de> Deserialize<'de> for Loose {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(LooseVisitor)
    }
}

struct LooseVisitor;

impl<'de> Visitor<'de> for LooseVisitor {
    type Value = Loose;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("any value")
    }

    fn visit_bool<E: de::Error>(self, _: bool) -> Result<Loose, E> {
        Ok(Loose::Other)
    }

    #[allow(clippy::cast_precision_loss)] // a share or an id never needs more
    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Loose, E> {
        Ok(Loose::Number(v as f64))
    }

    #[allow(clippy::cast_precision_loss)]
    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Loose, E> {
        Ok(Loose::Number(v as f64))
    }

    #[allow(clippy::cast_precision_loss)]
    fn visit_i128<E: de::Error>(self, v: i128) -> Result<Loose, E> {
        Ok(Loose::Number(v as f64))
    }

    #[allow(clippy::cast_precision_loss)]
    fn visit_u128<E: de::Error>(self, v: u128) -> Result<Loose, E> {
        Ok(Loose::Number(v as f64))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Loose, E> {
        Ok(Loose::Number(v))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Loose, E> {
        Ok(Loose::Text(v.to_owned()))
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<Loose, E> {
        Ok(Loose::Text(v))
    }

    fn visit_bytes<E: de::Error>(self, _: &[u8]) -> Result<Loose, E> {
        Ok(Loose::Other)
    }

    fn visit_none<E: de::Error>(self) -> Result<Loose, E> {
        Ok(Loose::Other)
    }

    fn visit_unit<E: de::Error>(self) -> Result<Loose, E> {
        Ok(Loose::Other)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Loose, D::Error> {
        Loose::deserialize(deserializer)
    }

    fn visit_newtype_struct<D: Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<Loose, D::Error> {
        Loose::deserialize(deserializer)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Loose, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element::<Loose>()? {
            items.push(item);
        }
        Ok(Loose::List(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Loose, A::Error> {
        while map.next_entry::<Loose, Loose>()?.is_some() {}
        Ok(Loose::Other)
    }

    fn visit_enum<A: de::EnumAccess<'de>>(self, data: A) -> Result<Loose, A::Error> {
        // A tagged value (`!thing value`): the value without its tag.
        let (_, variant) = data.variant::<Loose>()?;
        de::VariantAccess::newtype_variant::<Loose>(variant)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ADR-008: every form of YAML value is read, none fails.
    #[test]
    fn adr008_any_value_is_read() {
        for (text, expected) in [
            ("99999999999999999999999999", Loose::Number(1e26)),
            ("-5", Loose::Number(-5.0)),
            ("word", Loose::Text("word".to_owned())),
            (
                "[a, 1]",
                Loose::List(vec![Loose::Text("a".to_owned()), Loose::Number(1.0)]),
            ),
            ("{a: [1, {b: c}]}", Loose::Other),
            ("~", Loose::Other),
            ("true", Loose::Other),
            ("!thing 7", Loose::Number(7.0)),
        ] {
            assert_eq!(
                serde_norway::from_str::<Loose>(text).unwrap(),
                expected,
                "{text}"
            );
        }
    }
}
