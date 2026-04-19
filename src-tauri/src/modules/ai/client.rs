use crate::modules::ai::types::{AiConfig, ChatMessage, StreamEvent};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use serde_json::Value;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio_stream::StreamExt;

/// SSE 数据行中的 delta
#[derive(Deserialize)]
struct SseDelta {
    content: Option<String>,
}

#[derive(Deserialize)]
struct SseChoice {
    delta: SseDelta,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct SseChunk {
    choices: Vec<SseChoice>,
}

fn truncate_response_snippet(input: &str) -> String {
    const MAX_CHARS: usize = 240;
    let sanitized = input.split_whitespace().collect::<Vec<_>>().join(" ");
    if sanitized.chars().count() <= MAX_CHARS {
        sanitized
    } else {
        format!(
            "{}...",
            sanitized.chars().take(MAX_CHARS).collect::<String>()
        )
    }
}

fn body_looks_like_html(input: &str) -> bool {
    let trimmed = input.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    lower.starts_with("<!doctype html")
        || lower.starts_with("<html")
        || (lower.contains("<head") && lower.contains("<body"))
}

fn html_response_error(status: Option<reqwest::StatusCode>, body: &str) -> String {
    let prefix = match status {
        Some(status) => format!("API 返回了 HTML 页面（HTTP {}）", status),
        None => "API 返回了 HTML 页面".to_string(),
    };

    format!(
        "{}，这通常表示 Base URL 填成了网站首页而不是 OpenAI 兼容接口地址。请将“API Base URL”改为实际接口根地址，通常需要以 `/v1` 结尾；当前响应片段: {}",
        prefix,
        truncate_response_snippet(body)
    )
}

fn extract_text_content(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Array(parts) => {
            let combined = parts
                .iter()
                .filter_map(|part| match part {
                    Value::String(text) => Some(text.as_str()),
                    Value::Object(_) => part
                        .get("text")
                        .and_then(Value::as_str)
                        .or_else(|| part.get("content").and_then(Value::as_str)),
                    _ => None,
                })
                .collect::<String>();

            (!combined.is_empty()).then_some(combined)
        }
        _ => None,
    }
}

fn extract_non_stream_content(response: &Value) -> Option<String> {
    response
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| {
            choice
                .get("message")
                .and_then(|message| message.get("content"))
                .and_then(extract_text_content)
                .or_else(|| choice.get("text").and_then(extract_text_content))
        })
        .or_else(|| response.get("content").and_then(extract_text_content))
}

pub struct LlmClient {
    http: Client,
}

impl LlmClient {
    pub fn new() -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("Failed to build HTTP client");
        Self { http }
    }

    /// 流式调用，通过回调逐步返回 StreamEvent
    pub async fn stream_chat<F>(
        &self,
        config: &AiConfig,
        messages: &[ChatMessage],
        mut on_event: F,
        mut cancel_rx: Option<oneshot::Receiver<()>>,
    ) -> Result<String, String>
    where
        F: FnMut(StreamEvent),
    {
        let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));

        let req_body = json!({
            "model": config.model,
            "messages": messages,
            "temperature": config.temperature,
            "max_tokens": config.max_tokens,
            "stream": true
        });

        let mut request = self
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&req_body);

        // 根据 provider 设置认证头
        if !config.api_key.is_empty() {
            request = request.header("Authorization", format!("Bearer {}", config.api_key));
        }

        let response = request
            .send()
            .await
            .map_err(|e| format!("HTTP error: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("API error {}: {}", status, body));
        }

        let mut stream = response.bytes_stream();
        let mut full_content = String::new();
        let mut buffer = String::new();

        loop {
            let next_chunk = if let Some(cancel) = cancel_rx.as_mut() {
                tokio::select! {
                    _ = cancel => {
                        return Err("Request cancelled".to_string());
                    }
                    chunk = stream.next() => chunk,
                }
            } else {
                stream.next().await
            };

            let Some(chunk) = next_chunk else {
                break;
            };

            let chunk = chunk.map_err(|e| format!("Stream error: {}", e))?;
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            // 按行处理 SSE
            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim().to_string();
                buffer = buffer[pos + 1..].to_string();

                if line.starts_with("data: ") {
                    let data = &line["data: ".len()..];
                    if data == "[DONE]" {
                        return Ok(full_content);
                    }
                    if let Ok(chunk) = serde_json::from_str::<SseChunk>(data) {
                        if let Some(choice) = chunk.choices.first() {
                            if let Some(content) = &choice.delta.content {
                                full_content.push_str(content);
                                on_event(StreamEvent::Delta {
                                    content: content.clone(),
                                });
                            }
                            if choice.finish_reason.is_some() {
                                return Ok(full_content);
                            }
                        }
                    }
                }
            }
        }

        if let Some(cancel) = cancel_rx.as_mut() {
            if cancel.try_recv().is_ok() {
                return Err("Request cancelled".to_string());
            }
        }

        Ok(full_content)
    }

    /// 非流式调用（用于简单查询）
    pub async fn chat(
        &self,
        config: &AiConfig,
        messages: &[ChatMessage],
    ) -> Result<String, String> {
        let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));

        let req_body = json!({
            "model": config.model,
            "messages": messages,
            "temperature": config.temperature,
            "max_tokens": config.max_tokens,
            "stream": false
        });

        let mut request = self
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&req_body);

        if !config.api_key.is_empty() {
            request = request.header("Authorization", format!("Bearer {}", config.api_key));
        }

        let response = request
            .send()
            .await
            .map_err(|e| format!("HTTP error: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            if body_looks_like_html(&body) {
                return Err(html_response_error(Some(status), &body));
            }
            return Err(format!("API error {}: {}", status, body));
        }

        let body = response
            .text()
            .await
            .map_err(|e| format!("读取响应失败: {}", e))?;

        if body_looks_like_html(&body) {
            return Err(html_response_error(None, &body));
        }

        let resp: Value = serde_json::from_str(&body).map_err(|e| {
            format!(
                "响应解析失败: {}; 响应片段: {}",
                e,
                truncate_response_snippet(&body)
            )
        })?;

        extract_non_stream_content(&resp).ok_or_else(|| {
            format!(
                "未从模型响应中提取到文本内容，响应片段: {}",
                truncate_response_snippet(&body)
            )
        })
    }

    /// 测试连接是否可用
    pub async fn test_connection(&self, config: &AiConfig) -> Result<(), String> {
        let messages = vec![ChatMessage::user("ping")];
        // 最多等 10 秒
        let config_quick = AiConfig {
            max_tokens: 5,
            ..config.clone()
        };
        self.chat(&config_quick, &messages).await.map(|_| ())
    }
}

impl Default for LlmClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{body_looks_like_html, extract_non_stream_content, truncate_response_snippet};
    use serde_json::json;

    #[test]
    fn extracts_string_content_from_chat_completions_response() {
        let response = json!({
            "choices": [
                {
                    "message": {
                        "content": "pong"
                    }
                }
            ]
        });

        assert_eq!(
            extract_non_stream_content(&response).as_deref(),
            Some("pong")
        );
    }

    #[test]
    fn extracts_array_content_from_compatible_response() {
        let response = json!({
            "choices": [
                {
                    "message": {
                        "content": [
                            { "type": "text", "text": "pon" },
                            { "type": "text", "text": "g" }
                        ]
                    }
                }
            ]
        });

        assert_eq!(
            extract_non_stream_content(&response).as_deref(),
            Some("pong")
        );
    }

    #[test]
    fn extracts_top_level_content_when_choices_are_absent() {
        let response = json!({
            "content": [
                { "type": "text", "text": "pong" }
            ]
        });

        assert_eq!(
            extract_non_stream_content(&response).as_deref(),
            Some("pong")
        );
    }

    #[test]
    fn truncates_multiline_response_snippet_for_errors() {
        let snippet = truncate_response_snippet("line1\nline2\r\nline3");
        assert_eq!(snippet, "line1 line2 line3");
    }

    #[test]
    fn detects_html_responses() {
        assert!(body_looks_like_html(
            "<!doctype html><html><head><title>Gateway</title></head><body></body></html>"
        ));
        assert!(!body_looks_like_html("{\"choices\":[]}"));
    }
}
