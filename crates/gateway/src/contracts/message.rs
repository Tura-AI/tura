use serde::{ser::SerializeStruct, Deserialize, Serialize, Serializer};

#[derive(Debug, Clone, Deserialize)]
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: MessageRole,
    pub parts: Vec<MessagePart>,
    pub created_at: i64,
    pub updated_at: i64,
    pub parent_id: Option<String>,
}

impl Serialize for Message {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Message", 12)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("sessionID", &self.session_id)?;
        state.serialize_field("parentID", &self.parent_id)?;
        state.serialize_field("role", &self.role)?;
        state.serialize_field("parts", &self.parts)?;
        state.serialize_field(
            "time",
            &serde_json::json!({
                "created": self.created_at,
                "updated": self.updated_at,
            }),
        )?;
        state.serialize_field("created_at", &self.created_at)?;
        state.serialize_field("updated_at", &self.updated_at)?;
        let runtime = runtime_metrics_from_parts(&self.parts);
        state.serialize_field("cost", &runtime.cost)?;
        state.serialize_field(
            "providerID",
            &runtime
                .provider_id
                .unwrap_or_else(crate::session::manager::coding_agent_provider),
        )?;
        state.serialize_field("modelID", &runtime.model_id)?;
        state.serialize_field("tokens", &runtime.tokens)?;
        state.end()
    }
}

#[derive(Debug, Clone)]
struct RuntimeMessageMetrics {
    cost: f64,
    provider_id: Option<String>,
    model_id: Option<String>,
    tokens: serde_json::Value,
}

fn runtime_metrics_from_parts(parts: &[MessagePart]) -> RuntimeMessageMetrics {
    let mut input = 0_u64;
    let mut output = 0_u64;
    let mut reasoning = 0_u64;
    let mut cache_read = 0_u64;
    let mut cache_write = 0_u64;
    let mut cost = 0.0_f64;
    let mut provider_id = None;
    let mut model_id = None;

    for part in parts {
        let candidates = [
            part.metadata.as_ref(),
            part.state.as_ref().and_then(|state| state.get("metadata")),
        ];
        for metadata in candidates.into_iter().flatten() {
            let Some(usage) = metadata.get("usage") else {
                continue;
            };
            input = input.saturating_add(json_u64(usage, "input_tokens"));
            output = output.saturating_add(json_u64(usage, "output_tokens"));
            reasoning = reasoning.saturating_add(json_u64(usage, "reasoning_tokens"));
            cache_read = cache_read.saturating_add(json_u64(usage, "cached_input_tokens"));
            cache_write = cache_write.saturating_add(json_u64(usage, "cache_write_tokens"));
            cost += json_f64(usage, "total_cost");

            if provider_id.is_none() {
                provider_id = metadata
                    .get("provider")
                    .and_then(|provider| provider.get("provider_name"))
                    .and_then(|value| value.as_str())
                    .map(ToString::to_string);
            }
            if model_id.is_none() {
                model_id = metadata
                    .get("provider")
                    .and_then(|provider| provider.get("model_name"))
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToString::to_string);
            }
        }
    }

    RuntimeMessageMetrics {
        cost,
        provider_id,
        model_id,
        tokens: serde_json::json!({
            "input": input,
            "output": output,
            "reasoning": reasoning,
            "cache": {
                "read": cache_read,
                "write": cache_write,
            },
        }),
    }
}

fn json_u64(value: &serde_json::Value, key: &str) -> u64 {
    value.get(key).and_then(|value| value.as_u64()).unwrap_or(0)
}

fn json_f64(value: &serde_json::Value, key: &str) -> f64 {
    value
        .get(key)
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessagePart {
    pub id: String,
    pub session_id: String,
    pub message_id: String,
    #[serde(rename = "type")]
    pub part_type: String,
    pub content: Option<String>,
    pub text: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub call_id: Option<String>,
    pub tool: Option<String>,
    pub state: Option<serde_json::Value>,
}

impl Serialize for MessagePart {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("MessagePart", 10)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("sessionID", &self.session_id)?;
        state.serialize_field("messageID", &self.message_id)?;
        state.serialize_field("type", &self.part_type)?;
        state.serialize_field("content", &self.content)?;
        state.serialize_field("text", &self.text)?;
        state.serialize_field("metadata", &self.metadata)?;
        state.serialize_field("callID", &self.call_id)?;
        state.serialize_field("tool", &self.tool)?;
        state.serialize_field("state", &self.state)?;
        state.end()
    }
}

#[cfg(test)]
mod tests {
    use super::{Message, MessagePart, MessageRole};

    #[test]
    fn message_serialization_uses_runtime_usage_metadata() {
        let message = Message {
            id: "msg-1".to_string(),
            session_id: "session-1".to_string(),
            role: MessageRole::Assistant,
            parts: vec![MessagePart {
                id: "part-1".to_string(),
                session_id: "session-1".to_string(),
                message_id: "msg-1".to_string(),
                part_type: "tool".to_string(),
                content: None,
                text: None,
                metadata: Some(serde_json::json!({
                    "usage": {
                        "input_tokens": 10,
                        "output_tokens": 4,
                        "reasoning_tokens": 2,
                        "cached_input_tokens": 3,
                        "cache_write_tokens": 1,
                        "total_cost": 0.25
                    },
                    "provider": {
                        "provider_name": "openai",
                        "model_name": "gpt-test"
                    }
                })),
                call_id: Some("runtime-1".to_string()),
                tool: Some("runtime".to_string()),
                state: None,
            }],
            created_at: 1,
            updated_at: 2,
            parent_id: None,
        };

        let value = serde_json::to_value(message).expect("message should serialize");

        assert_eq!(value["tokens"]["input"], 10);
        assert_eq!(value["tokens"]["output"], 4);
        assert_eq!(value["tokens"]["reasoning"], 2);
        assert_eq!(value["tokens"]["cache"]["read"], 3);
        assert_eq!(value["tokens"]["cache"]["write"], 1);
        assert_eq!(value["cost"], 0.25);
        assert_eq!(value["providerID"], "openai");
        assert_eq!(value["modelID"], "gpt-test");
    }
}
