//! Worker-pool + chat-protocol type definitions, split by domain so
//! each file matches its responsibility. Re-exported so consumers can
//! keep their existing `crate::copilot::types::Foo` imports.

mod chat;
mod job;
mod worker_class;

pub(crate) use chat::{ChatAttachment, ChatDelta, ChatPrompt};
pub(crate) use job::{JobInput, JobOutput};
pub(crate) use worker_class::WorkerClass;
