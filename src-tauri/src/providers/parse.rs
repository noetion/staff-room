use crate::*;

pub(crate) fn parse_session_id(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            for key in [
                "thread_id",
                "threadId",
                "session_id",
                "sessionId",
                "conversation_id",
            ] {
                if let Some(Value::String(id)) = map.get(key) {
                    return Some(id.clone());
                }
            }
            map.values().find_map(parse_session_id)
        }
        Value::Array(values) => values.iter().find_map(parse_session_id),
        _ => None,
    }
}

pub(crate) fn json_u64(value: &Value, key: &str) -> Option<u64> {
    value
        .get(key)
        .and_then(|value| value.as_u64().or_else(|| value.as_str()?.parse().ok()))
}

pub(crate) fn json_f64(value: &Value, key: &str) -> Option<f64> {
    value
        .get(key)
        .and_then(|value| value.as_f64().or_else(|| value.as_str()?.parse().ok()))
}

pub(crate) fn first_u64(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| json_u64(value, key))
}

pub(crate) fn first_f64(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| json_f64(value, key))
}

pub(crate) fn merge_usage(target: &mut ProviderUsage, value: &Value) {
    for source in [
        Some(value),
        value.get("usage"),
        value.get("usage_info"),
        value.get("usageInfo"),
        value
            .get("message")
            .and_then(|message| message.get("usage")),
    ]
    .into_iter()
    .flatten()
    {
        target.input_tokens =
            first_u64(source, &["input_tokens", "inputTokens"]).or(target.input_tokens);
        target.cached_input_tokens = first_u64(
            source,
            &[
                "cached_input_tokens",
                "cachedInputTokens",
                "cache_read_input_tokens",
            ],
        )
        .or(target.cached_input_tokens);
        target.output_tokens =
            first_u64(source, &["output_tokens", "outputTokens"]).or(target.output_tokens);
        target.total_cost_usd =
            first_f64(source, &["total_cost_usd", "totalCostUsd"]).or(target.total_cost_usd);
        target.num_turns = first_u64(source, &["num_turns", "numTurns"]).or(target.num_turns);
    }
}

pub(crate) fn reported_model(value: &Value) -> Option<String> {
    ["model", "model_id", "modelId"]
        .iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .filter(|model| !model.trim().is_empty())
        .map(str::to_owned)
}

pub(crate) fn chat_response_is_complete(value: &str) -> bool {
    !value.trim().is_empty()
}

pub(crate) fn parse_result_text(value: &Value) -> Option<String> {
    if let Value::Object(map) = value {
        if let Some(Value::String(result)) = map.get("result") {
            return Some(result.clone());
        }
        if let Some(Value::Object(message)) = map.get("message") {
            if let Some(Value::Array(content)) = message.get("content") {
                let text = content
                    .iter()
                    .filter_map(|block| block.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("");
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }
    }
    None
}

pub(crate) fn text_blocks(value: &Value) -> Option<String> {
    let blocks = value.as_array()?;
    let text = blocks
        .iter()
        .filter_map(|block| {
            let block_type = block.get("type").and_then(Value::as_str);
            if block_type.is_some_and(|kind| !matches!(kind, "text" | "output_text")) {
                return None;
            }
            block
                .get("text")
                .and_then(Value::as_str)
                .or_else(|| block.get("content").and_then(Value::as_str))
        })
        .collect::<Vec<_>>()
        .join("");
    (!text.trim().is_empty()).then_some(text)
}

pub(crate) fn provider_chat_text(kind: &str, value: &Value) -> Option<String> {
    let event_type = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match kind {
        "codex" => {
            let item = value.get("item")?;
            (item.get("type").and_then(Value::as_str) == Some("agent_message"))
                .then(|| item.get("text").and_then(Value::as_str).map(str::to_owned))
                .flatten()
        }
        "claude" => match event_type {
            "assistant" => value
                .get("message")
                .and_then(|message| message.get("content"))
                .and_then(text_blocks),
            "result" => value
                .get("result")
                .and_then(Value::as_str)
                .map(str::to_owned),
            _ => None,
        },
        "cursor" => match event_type {
            "result" => value
                .get("result")
                .and_then(Value::as_str)
                .map(str::to_owned),
            _ => None,
        },
        _ => None,
    }
    .filter(|text| !text.trim().is_empty())
}

pub(crate) fn provider_chat_fragment(kind: &str, value: &Value) -> Option<String> {
    let event_type = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if kind == "cursor" && event_type == "assistant" {
        return value
            .get("message")
            .and_then(|message| message.get("content"))
            .and_then(text_blocks);
    }
    let text = match kind {
        "claude" if event_type == "stream_event" => value
            .get("event")
            .and_then(|event| event.get("delta"))
            .and_then(|delta| delta.get("text"))
            .and_then(Value::as_str),
        "cursor" if event_type == "text" => {
            value.get("text").and_then(Value::as_str).or_else(|| {
                value
                    .get("part")
                    .and_then(|part| part.get("text"))
                    .and_then(Value::as_str)
            })
        }
        _ => None,
    }?;
    (!text.is_empty()).then(|| text.to_owned())
}

pub(crate) fn provider_thinking_fragment<'a>(kind: &str, value: &'a Value) -> Option<&'a str> {
    if kind != "claude" || value.get("type").and_then(Value::as_str) != Some("stream_event") {
        return None;
    }
    let event = value.get("event")?;
    if event.get("type").and_then(Value::as_str) != Some("content_block_delta") {
        return None;
    }
    let delta = event.get("delta")?;
    (delta.get("type").and_then(Value::as_str) == Some("thinking_delta"))
        .then(|| delta.get("thinking").and_then(Value::as_str))
        .flatten()
        .filter(|text| !text.trim().is_empty())
}

#[derive(Default)]
pub(crate) struct ChatStreamBuffer {
    pub(crate) result_text: String,
    pending_text: String,
    pub(crate) thinking: String,
    pub(crate) thinking_updated: bool,
}

impl ChatStreamBuffer {
    pub(crate) fn push_fragment(&mut self, fragment: &str) {
        self.result_text.push_str(fragment);
        self.pending_text.push_str(fragment);
    }

    pub(crate) fn replace_snapshot(&mut self, text: String) {
        let delta = text
            .strip_prefix(&self.result_text)
            .unwrap_or(&text)
            .to_owned();
        self.result_text = text;
        self.pending_text.push_str(&delta);
    }

    pub(crate) fn push_thinking(&mut self, fragment: &str) {
        self.thinking.push_str(fragment);
        self.thinking_updated = true;
    }

    pub(crate) fn take_text_delta(&mut self) -> Option<String> {
        (!self.pending_text.is_empty()).then(|| std::mem::take(&mut self.pending_text))
    }
}

pub(crate) fn apply_provider_environment(command: &mut Command, repository: &Path) {
    command.env("NO_COLOR", "1");
    command
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "safe.directory")
        .env("GIT_CONFIG_VALUE_0", repository);
    hide_tokio_command_window(command);
}

pub(crate) fn provider_activity(kind: &str, value: &Value) -> Option<(String, String)> {
    let event_type = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match kind {
        "codex" if matches!(event_type, "item.started" | "item.completed") => {
            let item = value.get("item")?;
            let item_type = item.get("type").and_then(Value::as_str).unwrap_or_default();
            match item_type {
                "reasoning" => item
                    .get("text")
                    .and_then(Value::as_str)
                    .filter(|text| !text.trim().is_empty())
                    .map(|text| ("Thinking".to_owned(), truncate_utf8(text.trim(), 2 * 1024))),
                "command_execution" => item.get("command").and_then(Value::as_str).map(|command| {
                    let status = item
                        .get("status")
                        .and_then(Value::as_str)
                        .unwrap_or("running");
                    (
                        if status == "completed" {
                            "Command finished"
                        } else {
                            "Running command"
                        }
                        .to_owned(),
                        truncate_utf8(command, 1024),
                    )
                }),
                "file_change" => Some((
                    "Updating files".to_owned(),
                    "Applying repository changes in the managed worktree.".to_owned(),
                )),
                "mcp_tool_call" => Some((
                    "Using tool".to_owned(),
                    item.get("tool")
                        .or_else(|| item.get("name"))
                        .and_then(Value::as_str)
                        .unwrap_or("Provider tool")
                        .to_owned(),
                )),
                _ => None,
            }
        }
        "claude" if event_type == "stream_event" => {
            let event = value.get("event")?;
            let native_type = event
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if native_type == "content_block_start" {
                let block = event.get("content_block")?;
                if block.get("type").and_then(Value::as_str) == Some("tool_use") {
                    return Some((
                        "Using tool".to_owned(),
                        block
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("Claude tool")
                            .to_owned(),
                    ));
                }
            }
            if native_type == "content_block_delta" {
                let delta = event.get("delta")?;
                if delta.get("type").and_then(Value::as_str) == Some("thinking_delta") {
                    return delta
                        .get("thinking")
                        .and_then(Value::as_str)
                        .filter(|text| !text.trim().is_empty())
                        .map(|text| ("Thinking".to_owned(), truncate_utf8(text.trim(), 2 * 1024)));
                }
            }
            None
        }
        "cursor" => {
            let tool = value
                .get("tool")
                .or_else(|| value.get("name"))
                .and_then(Value::as_str);
            if event_type.contains("tool") || event_type.contains("command") {
                Some((
                    if event_type.contains("command") {
                        "Running command"
                    } else {
                        "Using tool"
                    }
                    .to_owned(),
                    tool.unwrap_or("Cursor tool").to_owned(),
                ))
            } else {
                None
            }
        }
        _ => None,
    }
}
