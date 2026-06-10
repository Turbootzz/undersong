//! A map that refuses duplicate keys at deserialization time.
//!
//! serde's default map visitor keeps the last value for a repeated key, so
//! a content file with conflicting entries (`Stone: Half, Stone: Double`)
//! would load silently and slip past the validator. Content errors must be
//! loud (CLAUDE.md golden rule 5): all map-shaped content schemas use this
//! type instead of a bare `BTreeMap`.

use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;

use serde::de::{Deserializer, Error as DeError, MapAccess, Visitor};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct UniqueMap<K: Ord, V>(BTreeMap<K, V>);

impl<K: Ord, V> std::ops::Deref for UniqueMap<K, V> {
    type Target = BTreeMap<K, V>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// Mutation cannot break the invariant: a BTreeMap structurally cannot hold
// duplicate keys; uniqueness only needs enforcing against the source text.
impl<K: Ord, V> std::ops::DerefMut for UniqueMap<K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<K: Ord, V> From<BTreeMap<K, V>> for UniqueMap<K, V> {
    fn from(map: BTreeMap<K, V>) -> Self {
        Self(map)
    }
}

impl<'de, K, V> Deserialize<'de> for UniqueMap<K, V>
where
    K: Deserialize<'de> + Ord + fmt::Debug,
    V: Deserialize<'de>,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct UniqueMapVisitor<K, V>(PhantomData<(K, V)>);

        impl<'de, K, V> Visitor<'de> for UniqueMapVisitor<K, V>
        where
            K: Deserialize<'de> + Ord + fmt::Debug,
            V: Deserialize<'de>,
        {
            type Value = UniqueMap<K, V>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a map with unique keys")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
                let mut map = BTreeMap::new();
                while let Some((key, value)) = access.next_entry::<K, V>()? {
                    let key_repr = format!("{key:?}");
                    if map.insert(key, value).is_some() {
                        return Err(A::Error::custom(format!("duplicate map key {key_repr}")));
                    }
                }
                Ok(UniqueMap(map))
            }
        }

        deserializer.deserialize_map(UniqueMapVisitor(PhantomData))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_keys_are_a_parse_error() {
        let result: Result<UniqueMap<String, u32>, _> = ron::from_str(r#"{"a": 1, "a": 2}"#);
        let err = result.expect_err("duplicate key must not parse");
        assert!(err.to_string().contains("duplicate map key"));
    }

    #[test]
    fn unique_keys_parse_and_read_back() {
        let map: UniqueMap<String, u32> = ron::from_str(r#"{"a": 1, "b": 2}"#).expect("parse");
        assert_eq!(map.get("a"), Some(&1));
        assert_eq!(map.get("b"), Some(&2));
        assert_eq!(map.len(), 2);
    }
}
