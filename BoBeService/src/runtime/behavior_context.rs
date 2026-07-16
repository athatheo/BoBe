use crate::services::souls_service::SoulsService;
use crate::services::user_profile_service::UserProfileService;
use std::sync::Arc;
use tracing::warn;

pub(crate) struct BehaviorContext {
    souls: Arc<SoulsService>,
    profiles: Arc<UserProfileService>,
}
impl BehaviorContext {
    pub(crate) fn new(souls: Arc<SoulsService>, profiles: Arc<UserProfileService>) -> Arc<Self> {
        Arc::new(Self { souls, profiles })
    }
    pub(crate) async fn prepend_to(&self, prompt: &str) -> String {
        let (souls, profiles) = tokio::join!(self.souls.list(true), self.profiles.list(true));
        let souls = souls.map_or_else(
            |e| {
                warn!(error=%e, "behavior_context.souls_unavailable");
                Vec::new()
            },
            |s| s.souls.into_iter().map(|v| v.content).collect(),
        );
        let profiles = profiles.map_or_else(
            |e| {
                warn!(error=%e, "behavior_context.profiles_unavailable");
                Vec::new()
            },
            |s| s.profiles.into_iter().map(|v| v.content).collect(),
        );
        format_prompt(&souls, &profiles, prompt)
    }
}
fn format_prompt(souls: &[String], profiles: &[String], prompt: &str) -> String {
    if souls.is_empty() && profiles.is_empty() {
        return prompt.to_owned();
    }
    let mut out = String::from("[bobe.behavior_context]\n");
    if !souls.is_empty() {
        out.push_str("Personality and behavior instructions:\n");
        out.push_str(&souls.join("\n\n"));
        out.push('\n');
    }
    if !profiles.is_empty() {
        out.push_str("Known user profile:\n");
        out.push_str(&profiles.join("\n\n"));
        out.push('\n');
    }
    out.push_str("[/bobe.behavior_context]\n\n");
    out.push_str(prompt);
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn context_precedes_turn() {
        let p = format_prompt(&["concise".into()], &["John".into()], "hi");
        assert!(p.starts_with("[bobe.behavior_context]"));
        assert!(p.ends_with("hi"));
    }
}
