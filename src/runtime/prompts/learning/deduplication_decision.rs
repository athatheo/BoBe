//! Prompts for LLM-based deduplication decisions.

use crate::llm::types::{AiMessage, ResponseFormat};
use crate::runtime::prompts::base::PromptConfig;

/// Prompt for deciding if a goal is a duplicate.
///
/// Given a candidate goal and potentially similar existing goals,
/// the LLM decides whether to CREATE, SKIP, or UPDATE.
pub struct GoalDeduplicationPrompt;

impl GoalDeduplicationPrompt {
    pub fn config() -> PromptConfig {
        PromptConfig {
            temperature: 0.0,
            max_tokens: 300,
            response_format: Some(ResponseFormat::json()),
            ..PromptConfig::default()
        }
    }

    fn system_message() -> &'static str {
        "You are a goal deduplication assistant. Your task is to determine if a candidate goal \
         is a duplicate of existing goals, or if it enriches one.\n\n\
         Rules for deciding:\n\
         1. CREATE - The goal represents a genuinely new objective not covered by existing goals\n\
         2. UPDATE - The goal covers the same objective as an existing goal but adds meaningful new \
         context, detail, or specificity that would improve the existing goal's description\n\
         3. SKIP - The goal is semantically equivalent to an existing goal (same intent, same \
         detail level, just different wording)\n\n\
         Use UPDATE when:\n\
         - The candidate adds concrete steps, timelines, or specifics to a vague existing goal\n\
         - The candidate narrows the scope with useful detail (e.g., existing: \"learn Python\", \
         candidate: \"learn Python async/await patterns for web scraping\")\n\
         - The candidate provides updated context about the same objective\n\n\
         Use SKIP when:\n\
         - The goals are truly identical in meaning and detail level\n\
         - One is just a rephrasing of the other with no new information\n\n\
         Consider goals as covering the same objective if:\n\
         - They have the same underlying intent (e.g., \"learn Python\" and \"master Python \
         programming\")\n\
         - One is a subset of another (e.g., \"learn web development\" covers \"learn HTML\")\n\n\
         Goals are NOT the same objective if:\n\
         - They have different specific outcomes\n\
         - They relate to different domains or contexts\n\
         - They have different scopes (unless one clearly contains the other)\n\n\
         Respond with a JSON object containing:\n\
         - decision: \"CREATE\", \"UPDATE\", or \"SKIP\"\n\
         - reason: Brief explanation (max 30 words)\n\
         - existing_goal_id: If UPDATE or SKIP, the ID of the matching existing goal (required)\n\
         - updated_content: If UPDATE, the enriched goal description merging old and new \
         context (required)"
    }

    /// Build messages for the deduplication decision.
    ///
    /// `existing_goals` is a list of `(id, content, priority)` tuples.
    pub fn messages(
        candidate_content: &str,
        existing_goals: &[(String, String, String)],
    ) -> Vec<AiMessage> {
        let user_content = if existing_goals.is_empty() {
            format!(
                "Candidate Goal: {candidate_content}\n\n\
                 Similar Existing Goals: None found\n\n\
                 Since no similar goals exist, this should be created."
            )
        } else {
            let existing_list: String = existing_goals
                .iter()
                .map(|(id, content, priority)| {
                    format!("- ID: {id}, Priority: {priority}, Content: {content}")
                })
                .collect::<Vec<_>>()
                .join("\n");

            format!(
                "Candidate Goal: {candidate_content}\n\n\
                 Similar Existing Goals:\n\
                 {existing_list}\n\n\
                 Decide whether to CREATE this as a new goal, UPDATE an existing goal with \
                 new context, or SKIP it as a duplicate."
            )
        };

        vec![
            AiMessage::system(Self::system_message()),
            AiMessage::user(user_content),
        ]
    }
}

/// Prompt for deciding if a memory is a duplicate.
///
/// Given a candidate memory and potentially similar existing memories,
/// the LLM decides whether to CREATE or SKIP.
pub struct MemoryDeduplicationPrompt;

impl MemoryDeduplicationPrompt {
    pub fn config() -> PromptConfig {
        PromptConfig {
            temperature: 0.0,
            max_tokens: 300,
            response_format: Some(ResponseFormat::json()),
            ..PromptConfig::default()
        }
    }

    fn system_message() -> &'static str {
        "You are a memory deduplication assistant. Your task is to determine if a candidate \
         memory should be stored or skipped.\n\n\
         Available actions:\n\
         1. CREATE - The memory contains new information not captured by existing memories\n\
         2. SKIP - The memory is semantically equivalent to an existing memory (no action \
         needed)\n\n\
         Decision guidelines:\n\n\
         Use CREATE when:\n\
         - This is genuinely new information not covered by existing memories\n\
         - It adds new specific details to a different aspect\n\n\
         Use SKIP when:\n\
         - The exact same information already exists\n\
         - An existing memory already captures this with equal or better detail\n\n\
         Respond with a JSON object containing:\n\
         - decision: \"CREATE\" or \"SKIP\"\n\
         - reason: Brief explanation (max 40 words)"
    }

    /// Build messages for the deduplication decision.
    ///
    /// `existing_memories` is a list of `(id, content, category)` tuples.
    pub fn messages(
        candidate_content: &str,
        candidate_category: &str,
        existing_memories: &[(String, String, String)],
    ) -> Vec<AiMessage> {
        let user_content = if existing_memories.is_empty() {
            format!(
                "Candidate Memory [{candidate_category}]: {candidate_content}\n\n\
                 Similar Existing Memories: None found\n\n\
                 Since no similar memories exist, this should be created."
            )
        } else {
            let existing_list: String = existing_memories
                .iter()
                .map(|(id, content, cat)| {
                    format!("- ID: {id}, Category: {cat}, Content: {content}")
                })
                .collect::<Vec<_>>()
                .join("\n");

            format!(
                "Candidate Memory [{candidate_category}]: {candidate_content}\n\n\
                 Similar Existing Memories:\n\
                 {existing_list}\n\n\
                 Decide whether to CREATE this as a new memory or SKIP it as a duplicate."
            )
        };

        vec![
            AiMessage::system(Self::system_message()),
            AiMessage::user(user_content),
        ]
    }
}
