//! Anthropic Messages API wire adapter (parity item 6, first slice).
//! Pure encoding/decoding: no network, credentials or spending. Live
//! transport stays disabled until the spend contract in
//! docs/agent-provider-protocol.md is satisfied (verified pricing,
//! budget journal settlement, explicit enabled configuration).

use crate::agent_protocol::{AgentMessage, ToolCall, validate_conversation};
use crate::agent_wire::Reply;
use serde_json::{Value, json};

/// Encode one conversation for POST /v1/messages. System messages are
/// joined into the top-level `system` field (the API has no system
/// role), consecutive tool results are grouped into a single user
/// message of tool_result blocks, and OpenAI-shaped tool schemas are
/// rewritten to Anthropic's input_schema form.
pub fn anthropic_request(
    model: &str,
    messages: &[AgentMessage],
    tools: &[Value],
    output_tokens: u32,
) -> Result<Value, String> {
    if model.trim().is_empty()
        || !(1..=8192).contains(&output_tokens)
        || tools.len() > 32
    {
        return Err("Invalid agent request limits".into());
    }
    let names: Vec<&str> = tools
        .iter()
        .map(|t| {
            t.pointer("/function/name")
                .and_then(Value::as_str)
                .ok_or("Tool name missing")
        })
        .collect::<Result<_, _>>()?;
    let unique: std::collections::BTreeSet<_> = names.iter().copied().collect();
    if unique.len() != names.len()
        || tools.iter().any(|t| {
            t.get("type").and_then(Value::as_str) != Some("function")
                || !t
                    .pointer("/function/parameters")
                    .is_some_and(Value::is_object)
        })
    {
        return Err("Invalid or duplicate tool schemas".into());
    }
    validate_conversation(messages, &names)?;

    let mut system_parts: Vec<&str> = Vec::new();
    let mut wire: Vec<Value> = Vec::new();
    let mut pending_results: Vec<Value> = Vec::new();
    // Flush accumulated tool_result blocks as one user message: the API
    // requires them grouped inside a user turn.
    macro_rules! flush_results {
        () => {
            if !pending_results.is_empty() {
                wire.push(json!({
                    "role": "user",
                    "content": std::mem::take(&mut pending_results),
                }));
            }
        };
    }
    for m in messages {
        match m {
            AgentMessage::System { content } => system_parts.push(content),
            AgentMessage::User { content } => {
                flush_results!();
                wire.push(json!({
                    "role": "user",
                    "content": [{"type": "text", "text": content}],
                }));
            }
            AgentMessage::Assistant { content, tool_calls } => {
                flush_results!();
                let mut blocks: Vec<Value> = Vec::new();
                if !content.is_empty() {
                    blocks.push(json!({"type": "text", "text": content}));
                }
                for c in tool_calls {
                    blocks.push(json!({
                        "type": "tool_use",
                        "id": c.id,
                        "name": c.name,
                        "input": c.arguments,
                    }));
                }
                if blocks.is_empty() {
                    return Err("Empty assistant message".into());
                }
                wire.push(json!({"role": "assistant", "content": blocks}));
            }
            AgentMessage::Tool {
                tool_call_id,
                content,
                error,
            } => {
                pending_results.push(json!({
                    "type": "tool_result",
                    "tool_use_id": tool_call_id,
                    "content": content,
                    "is_error": error,
                }));
            }
        }
    }
    flush_results!();
    if wire.first().and_then(|m| m.get("role")).and_then(Value::as_str)
        != Some("user")
    {
        return Err("Anthropic conversations must start with a user turn".into());
    }

    let tools: Vec<Value> = tools
        .iter()
        .map(|t| {
            json!({
                "name": t.pointer("/function/name"),
                "description": t.pointer("/function/description")
                    .cloned()
                    .unwrap_or(Value::String(String::new())),
                "input_schema": t.pointer("/function/parameters"),
            })
        })
        .collect();

    Ok(json!({
        "model": model,
        "max_tokens": output_tokens,
        "system": system_parts.join("\n\n"),
        "messages": wire,
        "tools": tools,
        "stream": false,
    }))
}

/// Decode one /v1/messages response. stop_reason must be end_turn or
/// tool_use; max_tokens/refusal/anything else is an incomplete or
/// rejected answer. stop_reason must agree with the presence of
/// tool_use blocks, and every call is structurally validated against
/// the registered tool names.
pub fn anthropic_reply(value: &Value, allowed: &[&str]) -> Result<Reply, String> {
    if value.get("error").is_some() {
        return Err("Provider returned an error".into());
    }
    if value.get("type").and_then(Value::as_str) != Some("message") {
        return Err("Not a message response".into());
    }
    let reason = value
        .get("stop_reason")
        .and_then(Value::as_str)
        .ok_or("Provider response not final")?;
    if !matches!(reason, "end_turn" | "tool_use") {
        return Err(format!("Incomplete or rejected agent response: {reason}"));
    }
    let blocks = value
        .get("content")
        .and_then(Value::as_array)
        .ok_or("Provider message missing")?;
    let mut texts: Vec<&str> = Vec::new();
    let mut calls = Vec::new();
    for block in blocks {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                texts.push(
                    block
                        .get("text")
                        .and_then(Value::as_str)
                        .ok_or("Text block missing text")?,
                );
            }
            Some("tool_use") => {
                calls.push(ToolCall {
                    id: block
                        .get("id")
                        .and_then(Value::as_str)
                        .ok_or("Tool ID missing")?
                        .into(),
                    name: block
                        .get("name")
                        .and_then(Value::as_str)
                        .ok_or("Tool name missing")?
                        .into(),
                    arguments: block
                        .get("input")
                        .filter(|v| v.is_object())
                        .cloned()
                        .ok_or("Invalid arguments")?,
                });
            }
            _ => return Err("Unsupported response block".into()),
        }
    }
    if (reason == "tool_use") != !calls.is_empty() {
        return Err("Stop reason disagrees with tool calls".into());
    }
    let content = texts.join("\n");
    // Supply empty correlated receipts solely for structural validation.
    // Not execution results.
    let mut validate = vec![AgentMessage::Assistant {
        content: content.clone(),
        tool_calls: calls.clone(),
    }];
    validate.extend(calls.iter().map(|c| AgentMessage::Tool {
        tool_call_id: c.id.clone(),
        content: String::new(),
        error: false,
    }));
    validate_conversation(&validate, allowed)?;
    Ok(Reply { content, calls })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tools() -> Vec<Value> {
        vec![json!({
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read one file",
                "parameters": {"type": "object"},
            },
        })]
    }

    #[test]
    fn request_maps_roles_tools_and_system() {
        let messages = vec![
            AgentMessage::System {
                content: "contract".into(),
            },
            AgentMessage::User {
                content: "explain a.rs".into(),
            },
        ];
        let req = anthropic_request("claude-sonnet-4-5", &messages, &tools(), 512)
            .unwrap();
        assert_eq!(req["system"], "contract");
        assert_eq!(req["max_tokens"], 512);
        assert_eq!(req["stream"], false);
        // No system role inside the message list; it starts with user.
        let msgs = req["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[0]["content"][0]["type"], "text");
        // OpenAI-shaped schema became input_schema form.
        assert_eq!(req["tools"][0]["name"], "read_file");
        assert!(req["tools"][0]["input_schema"].is_object());
        assert!(req["tools"][0].get("function").is_none());
    }

    #[test]
    fn tool_turns_become_grouped_tool_result_blocks() {
        let messages = vec![
            AgentMessage::System {
                content: "c".into(),
            },
            AgentMessage::User {
                content: "u".into(),
            },
            AgentMessage::Assistant {
                content: "looking".into(),
                tool_calls: vec![
                    ToolCall {
                        id: "a".into(),
                        name: "read_file".into(),
                        arguments: json!({"path": "a.rs"}),
                    },
                    ToolCall {
                        id: "b".into(),
                        name: "read_file".into(),
                        arguments: json!({"path": "b.rs"}),
                    },
                ],
            },
            AgentMessage::Tool {
                tool_call_id: "a".into(),
                content: "aaa".into(),
                error: false,
            },
            AgentMessage::Tool {
                tool_call_id: "b".into(),
                content: "bbb".into(),
                error: true,
            },
        ];
        let req =
            anthropic_request("claude-sonnet-4-5", &messages, &tools(), 512).unwrap();
        let msgs = req["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 3);
        // Assistant: text block then two tool_use blocks.
        let ab = msgs[1]["content"].as_array().unwrap();
        assert_eq!(ab[0]["type"], "text");
        assert_eq!(ab[1]["type"], "tool_use");
        assert_eq!(ab[1]["input"]["path"], "a.rs");
        assert_eq!(ab[2]["id"], "b");
        // Both results grouped into ONE user message.
        let ub = msgs[2]["content"].as_array().unwrap();
        assert_eq!(msgs[2]["role"], "user");
        assert_eq!(ub.len(), 2);
        assert_eq!(ub[0]["type"], "tool_result");
        assert_eq!(ub[0]["tool_use_id"], "a");
        assert_eq!(ub[1]["is_error"], true);
    }

    #[test]
    fn reply_parses_text_and_tool_use() {
        let text = anthropic_reply(
            &json!({
                "type": "message",
                "stop_reason": "end_turn",
                "content": [{"type": "text", "text": "done"}],
            }),
            &["read_file"],
        )
        .unwrap();
        assert_eq!(text.content, "done");
        assert!(text.calls.is_empty());

        let call = anthropic_reply(
            &json!({
                "type": "message",
                "stop_reason": "tool_use",
                "content": [
                    {"type": "text", "text": "checking"},
                    {"type": "tool_use", "id": "t1", "name": "read_file",
                     "input": {"path": "a.rs"}},
                ],
            }),
            &["read_file"],
        )
        .unwrap();
        assert_eq!(call.content, "checking");
        assert_eq!(call.calls[0].name, "read_file");
        assert_eq!(call.calls[0].arguments["path"], "a.rs");
    }

    #[test]
    fn reply_rejects_incomplete_refused_unknown_and_mismatched() {
        for reason in ["max_tokens", "refusal", ""] {
            assert!(anthropic_reply(
                &json!({"type": "message", "stop_reason": reason,
                        "content": [{"type": "text", "text": "x"}]}),
                &[]
            )
            .is_err());
        }
        // tool_use stop with no tool blocks, and the inverse.
        assert!(anthropic_reply(
            &json!({"type": "message", "stop_reason": "tool_use",
                    "content": [{"type": "text", "text": "x"}]}),
            &[]
        )
        .is_err());
        assert!(anthropic_reply(
            &json!({"type": "message", "stop_reason": "end_turn",
                    "content": [{"type": "tool_use", "id": "t",
                                 "name": "read_file", "input": {}}]}),
            &["read_file"]
        )
        .is_err());
        // Unregistered tool name.
        assert!(anthropic_reply(
            &json!({"type": "message", "stop_reason": "tool_use",
                    "content": [{"type": "tool_use", "id": "t",
                                 "name": "shell", "input": {}}]}),
            &["read_file"]
        )
        .is_err());
        // Provider error envelope.
        assert!(anthropic_reply(
            &json!({"type": "error", "error": {"type": "overloaded_error"}}),
            &[]
        )
        .is_err());
    }

    #[test]
    fn request_rejects_bad_limits_and_orphan_results() {
        assert!(anthropic_request("", &[], &tools(), 1).is_err());
        assert!(anthropic_request("m", &[], &tools(), 0).is_err());
        assert!(anthropic_request(
            "m",
            &[AgentMessage::User { content: "u".into() }],
            &tools(),
            8193
        )
        .is_err());
        // A bare tool result fails protocol validation, never encoding.
        assert!(anthropic_request(
            "m",
            &[AgentMessage::Tool {
                tool_call_id: "x".into(),
                content: "c".into(),
                error: false,
            }],
            &tools(),
            1
        )
        .is_err());
    }
}
