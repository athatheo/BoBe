//! Domain enum types — one per file to keep file names matching their
//! responsibility. Re-exported here so consumers can keep their existing
//! `crate::models::types::Foo` imports.

mod conversation_state;
mod goal_status;
mod turn_role;

pub(crate) use conversation_state::ConversationState;
pub(crate) use goal_status::GoalStatus;
pub(crate) use turn_role::TurnRole;
