use crate::modules::ai::types::{AiConfig, ChatMessage, StreamEvent};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;
use tokio_stream::StreamExt;

/// OpenAI 兼容请求体
#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    temperature: f32,
    max_tokens: u32,
    stream: bool,
}

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

/// 非流式响应
#[derive(Deserialize)]
struct NonStreamChoice {
    message: NonStreamMessage,
}

#[derive(Deserialize)]
struct NonStreamMessage {
    content: String,
}

#[derive(Deserialize)]
struct NonStreamResponse {
    choices: Vec<NonStreamChoice>,
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

        let response = request.send().await.map_err(|e| format!("HTTP error: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("API error {}: {}", status, body));
        }

        let mut stream = response.bytes_stream();
        let mut full_content = String::new();
        let mut buffer = String::new();

        while let Some(chunk) = stream.next().await {
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
                                on_event(StreamEvent::Delta { content: content.clone() });
                            }
                            if choice.finish_reason.is_some() {
                                return Ok(full_content);
                            }
                        }
                    }
                }
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

        let response = request.send().await.map_err(|e| format!("HTTP error: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("API error {}: {}", status, body));
        }

        let resp: NonStreamResponse = response.json().await.map_err(|e| e.to_string())?;
        resp.choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| "Empty response from LLM".to_string())
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
