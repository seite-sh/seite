use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::error::{PageError, Result};

#[derive(Debug, Clone)]
pub enum Provider {
    Claude,
    OpenAI,
}

pub struct AiClient {
    provider: Provider,
    api_key: String,
    model: String,
    client: Client,
}

pub struct GeneratedContent {
    pub title: String,
    pub body: String,
}

// Claude API types
#[derive(Serialize)]
struct ClaudeRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<Message>,
}

#[derive(Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ClaudeResponse {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    text: String,
}

// OpenAI API types
#[derive(Serialize)]
struct OpenAIRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
}

#[derive(Deserialize)]
struct OpenAIResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: MessageContent,
}

#[derive(Deserialize)]
struct MessageContent {
    content: String,
}

#[derive(Deserialize)]
struct ApiError {
    error: ApiErrorDetail,
}

#[derive(Deserialize)]
struct ApiErrorDetail {
    message: String,
}

const CONTENT_SYSTEM_PROMPT: &str = "You are a content writer for a static site. Generate well-structured markdown content. The first line MUST be a markdown H1 heading (starting with '# ') that serves as the title. Do not wrap the output in code fences.";

const TEMPLATE_SYSTEM_PROMPT: &str = "You are a web template designer. Generate a Tera/Jinja2 HTML template for a static site. The template MUST:
- Use {% extends \"base.html\" %} as the first line
- Define {% block title %} and {% block content %} blocks
- Use {{ page.title }}, {{ page.content | safe }}, {{ site.title }} variables
- Optionally use {{ page.date }}, {{ page.tags }}, {{ page.description }}, {{ page.url }}
- Be clean, semantic HTML with inline CSS styles
- Do not wrap the output in code fences
Output ONLY the template HTML, nothing else.";

impl AiClient {
    pub fn new(provider: Provider, api_key: String, model: String) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .unwrap();
        Self {
            provider,
            api_key,
            model,
            client,
        }
    }

    pub fn generate(&self, prompt: &str, content_type: &str) -> Result<GeneratedContent> {
        let user_msg = format!("Write a {content_type} about: {prompt}");
        let raw_text = self.call_llm(CONTENT_SYSTEM_PROMPT, &user_msg)?;
        let (title, body) = extract_title_and_body(&raw_text);
        Ok(GeneratedContent { title, body })
    }

    pub fn generate_template(&self, prompt: &str) -> Result<String> {
        let user_msg = format!("Create an HTML template for: {prompt}");
        self.call_llm(TEMPLATE_SYSTEM_PROMPT, &user_msg)
    }

    fn call_llm(&self, system_prompt: &str, user_msg: &str) -> Result<String> {
        match &self.provider {
            Provider::Claude => self.call_claude(system_prompt, user_msg),
            Provider::OpenAI => self.call_openai(system_prompt, user_msg),
        }
    }

    fn call_claude(&self, system_prompt: &str, user_msg: &str) -> Result<String> {
        let request = ClaudeRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            system: system_prompt.to_string(),
            messages: vec![Message {
                role: "user".into(),
                content: user_msg.into(),
            }],
        };

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            let msg = serde_json::from_str::<ApiError>(&body)
                .map(|e| e.error.message)
                .unwrap_or(body);
            return Err(PageError::Ai(format!("Claude API error ({status}): {msg}")));
        }

        let resp: ClaudeResponse = response.json().map_err(|e| {
            PageError::Ai(format!("failed to parse Claude response: {e}"))
        })?;

        resp.content
            .first()
            .map(|b| b.text.clone())
            .ok_or_else(|| PageError::Ai("empty response from Claude".into()))
    }

    fn call_openai(&self, system_prompt: &str, user_msg: &str) -> Result<String> {
        let request = OpenAIRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            messages: vec![
                Message {
                    role: "system".into(),
                    content: system_prompt.into(),
                },
                Message {
                    role: "user".into(),
                    content: user_msg.into(),
                },
            ],
        };

        let response = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            let msg = serde_json::from_str::<ApiError>(&body)
                .map(|e| e.error.message)
                .unwrap_or(body);
            return Err(PageError::Ai(format!(
                "OpenAI API error ({status}): {msg}"
            )));
        }

        let resp: OpenAIResponse = response.json().map_err(|e| {
            PageError::Ai(format!("failed to parse OpenAI response: {e}"))
        })?;

        resp.choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| PageError::Ai("empty response from OpenAI".into()))
    }
}

fn extract_title_and_body(text: &str) -> (String, String) {
    let text = text.trim();
    if let Some(first_line) = text.lines().next() {
        if first_line.starts_with("# ") {
            let title = first_line.trim_start_matches("# ").trim().to_string();
            let body = text[first_line.len()..].trim().to_string();
            return (title, body);
        }
    }
    let title = text
        .lines()
        .next()
        .unwrap_or("Untitled")
        .trim()
        .to_string();
    let body = text
        .lines()
        .skip(1)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    (title, body)
}
