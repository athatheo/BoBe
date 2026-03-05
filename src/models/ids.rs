use std::fmt;
use std::str::FromStr;

use uuid::Uuid;

macro_rules! define_id {
    ($name:ident) => {
        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            Hash,
            serde::Serialize,
            serde::Deserialize,
            sqlx::Type,
        )]
        #[serde(transparent)]
        #[sqlx(transparent)]
        pub(crate) struct $name(Uuid);

        #[allow(dead_code)]
        impl $name {
            pub(crate) fn new() -> Self {
                Self(Uuid::new_v4())
            }

            pub(crate) fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            pub(crate) fn as_uuid(&self) -> &Uuid {
                &self.0
            }

            pub(crate) fn into_uuid(self) -> Uuid {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl From<Uuid> for $name {
            fn from(uuid: Uuid) -> Self {
                Self(uuid)
            }
        }

        impl From<$name> for Uuid {
            fn from(id: $name) -> Self {
                id.0
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Uuid::from_str(s).map(Self)
            }
        }
    };
}

define_id!(ConversationId);
define_id!(ConversationTurnId);
define_id!(MemoryId);
define_id!(GoalId);
define_id!(ObservationId);
define_id!(SoulId);
define_id!(UserProfileId);
define_id!(AgentJobId);
define_id!(GoalPlanId);
define_id!(GoalPlanStepId);
define_id!(LearningStateId);
define_id!(CooldownId);

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn newtype_round_trips_through_uuid() {
        let id = ConversationId::new();
        let uuid = id.into_uuid();
        let back = ConversationId::from_uuid(uuid);
        assert_eq!(id, back);
    }

    #[test]
    fn newtype_display_matches_uuid() {
        let uuid = Uuid::new_v4();
        let id = MemoryId::from_uuid(uuid);
        assert_eq!(id.to_string(), uuid.to_string());
    }

    #[test]
    fn newtype_parse_from_string() {
        let uuid = Uuid::new_v4();
        let s = uuid.to_string();
        let id: GoalId = s.parse().unwrap();
        assert_eq!(id.into_uuid(), uuid);
    }

    #[test]
    fn newtype_serde_transparent() {
        let id = SoulId::new();
        let json = serde_json::to_string(&id).unwrap();
        let back: SoulId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
        // Should serialize as bare UUID string, not wrapped object
        assert_eq!(json, format!("\"{}\"", id));
    }

    #[test]
    fn different_id_types_are_incompatible() {
        // This test verifies the types exist and are distinct.
        // Compile-time safety: you can't pass a GoalId where a MemoryId is expected.
        let _goal = GoalId::new();
        let _memory = MemoryId::new();
        // If these were both Uuid, they'd be interchangeable.
    }
}
