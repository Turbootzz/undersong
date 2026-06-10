//! Newtype ids for content references.
//!
//! Content is data (RON), so cross-references are strings — but inside code
//! they are distinct types, so a move id can never be passed where a species
//! id is expected.

macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(
            Debug,
            Clone,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            serde::Serialize,
            serde::Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(raw: impl Into<String>) -> Self {
                Self(raw.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<&str> for $name {
            fn from(raw: &str) -> Self {
                Self(raw.to_owned())
            }
        }
    };
}

define_id!(
    /// Species (Motif) id, e.g. `fanfyre`.
    SpeciesId
);
define_id!(
    /// Move id, e.g. `ember_note`.
    MoveId
);
define_id!(
    /// Ability id, e.g. `crescendo_ember`.
    AbilityId
);
define_id!(
    /// Item id, e.g. `fermata`.
    ItemId
);
define_id!(
    /// Trainer id, e.g. `maestro_mirelle`.
    TrainerId
);
define_id!(
    /// Map id, e.g. `prelude_town`.
    MapId
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_distinct_types_with_string_views() {
        let species = SpeciesId::new("fanfyre");
        assert_eq!(species.as_str(), "fanfyre");
        assert_eq!(species.to_string(), "fanfyre");
        assert_eq!(SpeciesId::from("fanfyre"), species);
    }

    #[test]
    fn ids_serialize_transparently_as_strings() {
        let id = MoveId::new("ember_note");
        let ron = ron::to_string(&id).expect("serialize");
        assert_eq!(ron, "\"ember_note\"");
        let back: MoveId = ron::from_str(&ron).expect("deserialize");
        assert_eq!(back, id);
    }
}
