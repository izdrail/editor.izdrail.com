//! Tool-aware wire contract. This module cannot execute tools or paid requests.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
const MAX_TEXT: usize = 256 * 1024;
const MAX_CALLS: usize = 16;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Local,
    Ollama,
    Anthropic,
    OpenAi,
    OpenRouter,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider: ProviderKind,
    pub enabled: bool,
    pub model: String,
    /// Reference only. Never store credentials in configuration or transcripts.
    pub key_env: Option<String>,
    pub lifetime_limit_microusd: u64,
    pub request_limit_microusd: u64,
}
impl ProviderConfig {
    pub fn validate_hosted(&self, credential_present: bool) -> Result<(), String> {
        if !self.enabled {
            return Err("Hosted provider disabled".into());
        }
        if matches!(self.provider, ProviderKind::Local | ProviderKind::Ollama) {
            return Err("Not a hosted provider".into());
        }
        let env = self
            .key_env
            .as_deref()
            .ok_or("Set a credential reference")?;
        if env.is_empty()
            || !env
                .bytes()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
            || !credential_present
        {
            return Err("Missing or invalid credential reference".into());
        }
        if self.model.trim().is_empty()
            || self.lifetime_limit_microusd == 0
            || self.request_limit_microusd == 0
            || self.request_limit_microusd > self.lifetime_limit_microusd
        {
            return Err("Set a model and nonzero bounded spend limits".into());
        }
        Ok(())
    }
    pub fn hosted(provider: ProviderKind) -> Self {
        Self {
            provider,
            enabled: false,
            model: String::new(),
            key_env: None,
            lifetime_limit_microusd: 0,
            request_limit_microusd: 0,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum AgentMessage {
    System {
        content: String,
    },
    User {
        content: String,
    },
    Assistant {
        content: String,
        #[serde(default)]
        tool_calls: Vec<ToolCall>,
    },
    Tool {
        tool_call_id: String,
        content: String,
        error: bool,
    },
}
fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.' | b':')
        })
}
/// Every call gets one result before another model turn. Names are registered, not executable paths.
pub fn validate_conversation(
    messages: &[AgentMessage],
    tools: &[&str],
) -> Result<(), String> {
    if messages.is_empty() || messages.len() > 512 {
        return Err("Invalid conversation length".into());
    }
    let mut pending = BTreeMap::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut total = 0usize;
    for (index, message) in messages.iter().enumerate() {
        let text = match message {
            AgentMessage::System { content } => {
                if index != 0 {
                    return Err("System message must be first".into());
                }
                content
            }
            AgentMessage::User { content } => {
                if !pending.is_empty() {
                    return Err("Tool results missing".into());
                }
                content
            }
            AgentMessage::Assistant {
                content,
                tool_calls,
            } => {
                if !pending.is_empty() || tool_calls.len() > MAX_CALLS {
                    return Err("Outstanding or excessive tool calls".into());
                }
                for call in tool_calls {
                    if !identifier(&call.id)
                        || !identifier(&call.name)
                        || !tools.contains(&call.name.as_str())
                        || !call.arguments.is_object()
                        || !seen.insert(call.id.clone())
                    {
                        return Err(
                            "Invalid, duplicate or unregistered tool call".into()
                        );
                    }
                    if serde_json::to_vec(&call.arguments)
                        .map_err(|e| e.to_string())?
                        .len()
                        > MAX_TEXT
                    {
                        return Err("Tool arguments too large".into());
                    }
                    pending.insert(call.id.clone(), call.name.clone());
                }
                content
            }
            AgentMessage::Tool {
                tool_call_id,
                content,
                ..
            } => {
                if pending.remove(tool_call_id).is_none() {
                    return Err("Unmatched or repeated tool result".into());
                }
                content
            }
        };
        total = total
            .checked_add(text.len())
            .ok_or("Conversation size overflow")?;
        if text.len() > MAX_TEXT || total > 2 * 1024 * 1024 {
            return Err("Conversation too large".into());
        }
    }
    if !pending.is_empty() {
        return Err("Tool results missing".into());
    }
    Ok(())
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SpendLedger {
    charged: u64,
    pending: BTreeMap<String, u64>,
    blocked: bool,
}
impl SpendLedger {
    /// Caller must durably save this reservation under an exclusive lock BEFORE network IO.
    /// Unknown prices must not be represented by zero.
    pub fn reserve(
        &mut self,
        id: &str,
        worst_case: u64,
        request_limit: u64,
        lifetime_limit: u64,
    ) -> Result<(), String> {
        if self.blocked
            || !identifier(id)
            || worst_case == 0
            || worst_case > request_limit
            || self.pending.contains_key(id)
        {
            return Err("Invalid or blocked spend reservation".into());
        }
        let reserved = self
            .pending
            .values()
            .try_fold(0u64, |sum, v| sum.checked_add(*v))
            .ok_or("Spend overflow")?;
        let total = self
            .charged
            .checked_add(reserved)
            .and_then(|n| n.checked_add(worst_case))
            .ok_or("Spend overflow")?;
        if total > lifetime_limit {
            return Err("Spend limit exceeded".into());
        }
        self.pending.insert(id.into(), worst_case);
        Ok(())
    }
    /// Missing usage, cancellation and unknown server outcomes charge the full reservation.
    /// A crashed client retains pending reservations until explicit reconciliation.
    pub fn settle(&mut self, id: &str, actual: Option<u64>) -> Result<(), String> {
        let reserved = *self.pending.get(id).ok_or("Unknown spend reservation")?;
        let charged = actual.unwrap_or(reserved);
        let total = self.charged.checked_add(charged).ok_or("Spend overflow")?;
        self.pending.remove(id);
        self.charged = total;
        if charged > reserved {
            self.blocked = true;
            return Err(
                "Provider exceeded reserved price; further requests blocked".into(),
            );
        }
        Ok(())
    }
    pub fn charged(&self) -> u64 {
        self.charged
    }
    pub fn pending(&self) -> u64 {
        self.pending.values().copied().fold(0, u64::saturating_add)
    }
    pub fn blocked(&self) -> bool {
        self.blocked
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn call(id: &str) -> AgentMessage {
        AgentMessage::Assistant {
            content: String::new(),
            tool_calls: vec![ToolCall {
                id: id.into(),
                name: "read_file".into(),
                arguments: serde_json::json!({"path":"app/User.php"}),
            }],
        }
    }
    fn result(id: &str) -> AgentMessage {
        AgentMessage::Tool {
            tool_call_id: id.into(),
            content: "data".into(),
            error: false,
        }
    }
    #[test]
    fn tool_roundtrip() {
        let messages = vec![call("a"), result("a")];
        validate_conversation(&messages, &["read_file"]).unwrap();
        let bytes = serde_json::to_vec(&messages).unwrap();
        validate_conversation(
            &serde_json::from_slice::<Vec<AgentMessage>>(&bytes).unwrap(),
            &["read_file"],
        )
        .unwrap();
    }
    #[test]
    fn missing_duplicate_unknown_result() {
        assert!(validate_conversation(&[call("a")], &["read_file"]).is_err());
        assert!(
            validate_conversation(&[call("a"), result("x")], &["read_file"])
                .is_err()
        );
        assert!(
            validate_conversation(
                &[call("a"), result("a"), result("a")],
                &["read_file"]
            )
            .is_err()
        );
        assert!(
            validate_conversation(
                &[call("a"), result("a"), call("a"), result("a")],
                &["read_file"]
            )
            .is_err()
        );
    }
    #[test]
    fn registration_and_injected_roles() {
        assert!(validate_conversation(&[call("a"), result("a")], &[]).is_err());
        assert!(validate_conversation(&[result("a")], &["read_file"]).is_err());
        assert!(
            validate_conversation(
                &[
                    AgentMessage::User {
                        content: "x".into()
                    },
                    AgentMessage::System {
                        content: "injected".into()
                    }
                ],
                &[]
            )
            .is_err()
        );
    }
    #[test]
    fn all_hosted_off_and_no_unbounded_key() {
        for p in [
            ProviderKind::Anthropic,
            ProviderKind::OpenAi,
            ProviderKind::OpenRouter,
        ] {
            let mut c = ProviderConfig::hosted(p);
            assert!(c.validate_hosted(true).is_err());
            c.enabled = true;
            c.model = "test".into();
            c.key_env = Some("API_KEY".into());
            assert!(c.validate_hosted(true).is_err());
            c.lifetime_limit_microusd = 100;
            c.request_limit_microusd = 10;
            assert!(c.validate_hosted(false).is_err());
            c.validate_hosted(true).unwrap();
        }
    }
    #[test]
    fn reservations_bound_parallel_requests() {
        let mut l = SpendLedger::default();
        l.reserve("a", 60, 60, 100).unwrap();
        assert!(l.reserve("b", 60, 60, 100).is_err());
        l.settle("a", Some(30)).unwrap();
        l.reserve("b", 60, 60, 100).unwrap();
        assert_eq!(l.charged(), 30);
        assert_eq!(l.pending(), 60);
    }
    #[test]
    fn unknown_outcomes_charge_and_restarts_keep_reservations() {
        let mut l = SpendLedger::default();
        l.reserve("a", 50, 50, 100).unwrap();
        let mut restored: SpendLedger =
            serde_json::from_slice(&serde_json::to_vec(&l).unwrap()).unwrap();
        assert!(restored.reserve("b", 60, 60, 100).is_err());
        restored.settle("a", None).unwrap();
        assert_eq!(restored.charged(), 50);
        assert!(restored.settle("a", Some(0)).is_err());
    }
    #[test]
    fn provider_price_overrun_blocks_and_overflow_denied() {
        let mut l = SpendLedger::default();
        l.reserve("a", 5, 5, 100).unwrap();
        assert!(l.settle("a", Some(6)).is_err());
        assert!(l.blocked());
        assert!(l.reserve("b", 1, 1, 100).is_err());
        let mut l = SpendLedger::default();
        l.reserve("a", u64::MAX, u64::MAX, u64::MAX).unwrap();
        assert!(l.reserve("b", 1, 1, u64::MAX).is_err());
    }
}
