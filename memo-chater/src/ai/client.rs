//! AI客户端封装

use crate::config::AiConfig;
use crate::types::ChatMessage;
use thiserror::Error;

/// AI 调用错误
#[derive(Debug, Error)]
pub enum AiError {
    #[error("网络错误: {0}")]
    NetworkError(String),
    #[error("API错误: {0}")]
    ApiError(String),
    #[error("解析错误: {0}")]
    ParseError(String),
    #[error("配置错误: {0}")]
    ConfigError(String),
}
use futures::Stream;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use regex::Regex;
use once_cell::sync::Lazy;

/// 用于匹配思考标签的正则表达式（支持 <think>、<thinking> 等变体）
static THINKING_TAG_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?s)<think(?:ing)?[^>]*>.*?</think(?:ing)?>").unwrap()
});

/// AI客户端
#[derive(Clone)]
pub struct AiClient {
    http_client: Client,
    api_base: String,
    api_key: String,
    model: String,
    embedding_model: Option<String>,
    /// 禁用 Gemini 思考功能
    disable_gemini_thinking: bool,
}

impl AiClient {
    pub fn new(api_base: String, api_key: String, model: String) -> Self {
        let http_client = Client::builder()
            .no_proxy()
            .build()
            .unwrap_or_else(|_| Client::new());
        
        Self {
            http_client,
            api_base,
            api_key,
            model,
            embedding_model: None,
            disable_gemini_thinking: false,
        }
    }
    
    /// 设置是否禁用 Gemini 思考
    pub fn with_disable_gemini_thinking(mut self, disable: bool) -> Self {
        self.disable_gemini_thinking = disable;
        self
    }

    /// 清理AI响应中的思考标签（如 <think>...</think>、<thinking>...</thinking>）
    /// 某些思考模型会输出思考过程，这个方法用于移除这些内容
    pub fn strip_thinking_tags(content: &str) -> String {
        let result = THINKING_TAG_REGEX.replace_all(content, "");
        result.trim().to_string()
    }

    /// 设置 embedding 模型
    pub fn with_embedding_model(mut self, model: String) -> Self {
        self.embedding_model = Some(model);
        self
    }

    /// 从配置创建客户端
    pub fn from_config(config: &crate::config::AiApiConfig) -> Result<Self, AiError> {
        let api_key = config.get_api_key().unwrap_or_default();
        
        Ok(Self {
            http_client: Client::builder()
                .no_proxy()
                .build()
                .unwrap_or_else(|_| Client::new()),
            api_base: config.api_base.clone(),
            api_key,
            model: config.main_model.clone(),
            embedding_model: Some(config.embedding_model.clone()),
            disable_gemini_thinking: config.disable_gemini_thinking,
        })
    }

    /// 从配置创建处理器客户端（使用 processor_model）
    pub fn processor_from_config(config: &AiConfig) -> Result<Self, AiError> {
        let api_key = config.get_api_key().unwrap_or_default();
        
        Ok(Self {
            http_client: Client::builder()
                .no_proxy()
                .build()
                .unwrap_or_else(|_| Client::new()),
            api_base: config.api_base.clone(),
            api_key,
            model: config.processor_model.clone(),
            embedding_model: Some(config.embedding_model.clone()),
            disable_gemini_thinking: false,
        })
    }

    /// 生成文本的向量嵌入
    pub async fn embedding(&self, text: &str) -> Result<Vec<f32>, AiError> {
        self.embedding_with_model(text, None).await
    }

    /// 生成文本的向量嵌入（支持指定模型）
    pub async fn embedding_with_model(&self, text: &str, model: Option<&str>) -> Result<Vec<f32>, AiError> {
        let model = model
            .map(|m| m.to_string())
            .or_else(|| self.embedding_model.clone())
            .ok_or_else(|| AiError::ApiError("未配置 embedding 模型".to_string()))?;
        
        let url = format!("{}/embeddings", self.api_base.trim_end_matches('/'));
        
        let request_body = EmbeddingRequest {
            model: model.clone(),
            input: text.to_string(),
        };

        tracing::debug!("AiClient::embedding - url={}, model={}", url, model);

        let mut builder = self.http_client.post(&url);
        
        if !self.api_key.is_empty() {
            builder = builder.header("Authorization", format!("Bearer {}", self.api_key));
        }
        
        let response = builder
            .json(&request_body)
            .send()
            .await
            .map_err(|e| AiError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AiError::ApiError(format!(
                "Embedding API返回错误 {}: {}",
                status, error_text
            )));
        }

        let embedding_response: EmbeddingResponse = response
            .json()
            .await
            .map_err(|e| AiError::ParseError(e.to_string()))?;

        embedding_response
            .data
            .first()
            .map(|d| d.embedding.clone())
            .ok_or_else(|| AiError::ParseError("响应中没有 embedding 数据".to_string()))
    }

    /// 批量生成向量嵌入
    pub async fn embedding_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, AiError> {
        self.embedding_batch_with_model(texts, None).await
    }

    /// 批量生成向量嵌入（支持指定模型）
    pub async fn embedding_batch_with_model(&self, texts: &[String], model: Option<&str>) -> Result<Vec<Vec<f32>>, AiError> {
        let model = model
            .map(|m| m.to_string())
            .or_else(|| self.embedding_model.clone())
            .ok_or_else(|| AiError::ApiError("未配置 embedding 模型".to_string()))?;
        
        let url = format!("{}/embeddings", self.api_base.trim_end_matches('/'));
        
        let request_body = EmbeddingBatchRequest {
            model: model.clone(),
            input: texts.to_vec(),
        };

        tracing::debug!("AiClient::embedding_batch - url={}, model={}, count={}", url, model, texts.len());

        let mut builder = self.http_client.post(&url);
        
        if !self.api_key.is_empty() {
            builder = builder.header("Authorization", format!("Bearer {}", self.api_key));
        }
        
        let response = builder
            .json(&request_body)
            .send()
            .await
            .map_err(|e| AiError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AiError::ApiError(format!(
                "Embedding API返回错误 {}: {}",
                status, error_text
            )));
        }

        let embedding_response: EmbeddingResponse = response
            .json()
            .await
            .map_err(|e| AiError::ParseError(e.to_string()))?;

        Ok(embedding_response.data.into_iter().map(|d| d.embedding).collect())
    }

    /// 非流式聊天
    pub async fn chat(&self, messages: &[ChatMessage]) -> Result<String, AiError> {
        self.chat_with_model(messages, None).await
    }

    /// 非流式聊天（指定模型）
    pub async fn chat_with_model(&self, messages: &[ChatMessage], model: Option<&str>) -> Result<String, AiError> {
        let url = format!("{}/chat/completions", self.api_base.trim_end_matches('/'));
        let use_model = model.unwrap_or(&self.model);
        
        // 检测是否为 Gemini 模型并需要禁用思考
        let thinking_config = if self.disable_gemini_thinking && use_model.to_lowercase().contains("gemini") {
            tracing::info!("检测到 Gemini 模型且已启用禁用思考，添加 thinking_budget: 0");
            Some(ThinkingConfig { thinking_budget: 0 })
        } else {
            None
        };

        let request_body = ChatRequest {
            model: use_model.to_string(),
            messages: messages.iter().map(|m| ApiMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            }).collect(),
            stream: false,
            thinking_config,
        };

        tracing::debug!(
            "AiClient::chat - url={}, model={}, messages={}",
            url, use_model, messages.len()
        );

        let mut builder = self.http_client.post(&url);
        
        if !self.api_key.is_empty() {
            builder = builder.header("Authorization", format!("Bearer {}", self.api_key));
        }
        
        let response = builder
            .json(&request_body)
            .send()
            .await
            .map_err(|e| AiError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AiError::ApiError(format!(
                "API返回错误 {}: {}",
                status, error_text
            )));
        }

        let chat_response: ChatResponse = response
            .json()
            .await
            .map_err(|e| AiError::ParseError(e.to_string()))?;

        chat_response
            .choices
            .first()
            .map(|c| Self::strip_thinking_tags(&c.message.content))
            .ok_or_else(|| AiError::ParseError("响应中没有选项".to_string()))
    }

    /// 流式聊天
    pub async fn chat_stream(
        &self,
        messages: &[ChatMessage],
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, AiError>> + Send>>, AiError> {
        self.chat_stream_with_model(messages, None).await
    }

    /// 流式聊天（指定模型）
    pub async fn chat_stream_with_model(
        &self,
        messages: &[ChatMessage],
        model: Option<&str>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, AiError>> + Send>>, AiError> {
        let url = format!("{}/chat/completions", self.api_base.trim_end_matches('/'));
        let use_model = model.unwrap_or(&self.model);
        
        // 检测是否为 Gemini 模型并需要禁用思考
        let thinking_config = if self.disable_gemini_thinking && use_model.to_lowercase().contains("gemini") {
            tracing::info!("检测到 Gemini 模型且已启用禁用思考，添加 thinking_budget: 0");
            Some(ThinkingConfig { thinking_budget: 0 })
        } else {
            None
        };

        let request_body = ChatRequest {
            model: use_model.to_string(),
            messages: messages.iter().map(|m| ApiMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            }).collect(),
            stream: true,
            thinking_config,
        };

        let has_key = !self.api_key.is_empty();
        tracing::debug!(
            "AiClient::chat_stream - url={}, model={}, messages={}, has_key={}, key_len={}",
            url, self.model, messages.len(), has_key, self.api_key.len()
        );

        let body_json = serde_json::to_string(&request_body)
            .map_err(|e| AiError::ParseError(e.to_string()))?;
        
        let preview: String = body_json.chars().take(100).collect();
        tracing::debug!("Request body: {}", preview);
        
        let auth_header = format!("Bearer {}", self.api_key);
        
        let request = self.http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", &auth_header)
            .body(body_json)
            .build()
            .map_err(|e| AiError::NetworkError(e.to_string()))?;
        
        tracing::info!("=== 实际请求信息 ===");
        tracing::info!("URL: {}", request.url());
        tracing::info!("Method: {}", request.method());
        for (name, value) in request.headers() {
            tracing::info!("Header: {} = {:?}", name, value);
        }
        tracing::info!("===================");
        
        let response = self.http_client
            .execute(request)
            .await
            .map_err(|e| AiError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AiError::ApiError(format!(
                "API返回错误 {}: {}",
                status, error_text
            )));
        }

        tracing::info!("开始处理流式响应...");
        
        let stream = async_stream::stream! {
            use futures::StreamExt;
            
            let mut stream = response.bytes_stream();
            let mut buffer = String::new();
            let mut chunk_count = 0;
            
            // 思考标签过滤状态
            let mut in_thinking_tag = false;
            let mut tag_buffer = String::new();
            
            tracing::debug!("进入流式循环");
            
            while let Some(chunk) = stream.next().await {
                chunk_count += 1;
                match chunk {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        buffer.push_str(&text);
                        
                        while let Some(pos) = buffer.find('\n') {
                            let line = buffer[..pos].trim().to_string();
                            buffer = buffer[pos + 1..].to_string();
                            
                            if line.is_empty() || line == "data: [DONE]" {
                                continue;
                            }
                            
                            if let Some(json_str) = line.strip_prefix("data: ") {
                                match serde_json::from_str::<StreamChunk>(json_str) {
                                    Ok(chunk) => {
                                        if let Some(choice) = chunk.choices.first() {
                                            if let Some(content) = &choice.delta.content {
                                                // 过滤思考标签
                                                for ch in content.chars() {
                                                    if ch == '<' {
                                                        tag_buffer.clear();
                                                        tag_buffer.push(ch);
                                                    } else if !tag_buffer.is_empty() {
                                                        tag_buffer.push(ch);
                                                        
                                                        if tag_buffer.starts_with("<think") && ch == '>' {
                                                            in_thinking_tag = true;
                                                            tag_buffer.clear();
                                                        } else if in_thinking_tag && tag_buffer.ends_with("</think>") {
                                                            in_thinking_tag = false;
                                                            tag_buffer.clear();
                                                        } else if in_thinking_tag && tag_buffer.ends_with("</thinking>") {
                                                            in_thinking_tag = false;
                                                            tag_buffer.clear();
                                                        } else if !in_thinking_tag && !tag_buffer.starts_with("<think") && ch == '>' {
                                                            yield Ok(tag_buffer.clone());
                                                            tag_buffer.clear();
                                                        }
                                                    } else if !in_thinking_tag {
                                                        yield Ok(ch.to_string());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Err(_) => {}
                                }
                            }
                        }
                    }
                    Err(e) => {
                        yield Err(AiError::NetworkError(e.to_string()));
                    }
                }
            }
            
            tracing::debug!("流式循环结束，共处理 {} 个chunk", chunk_count);
        };

        Ok(Box::pin(stream))
    }
}

// ============ 请求/响应结构 ============

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ApiMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking_config: Option<ThinkingConfig>,
}

/// Gemini 思考配置
#[derive(Debug, Serialize)]
struct ThinkingConfig {
    /// 思考预算（0 = 禁用思考）
    thinking_budget: i32,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ApiMessage,
}

#[derive(Debug, Deserialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
}

#[derive(Debug, Deserialize)]
struct StreamDelta {
    content: Option<String>,
}

// ============ Embedding 请求/响应结构 ============

#[derive(Debug, Serialize)]
struct EmbeddingRequest {
    model: String,
    input: String,
}

#[derive(Debug, Serialize)]
struct EmbeddingBatchRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}