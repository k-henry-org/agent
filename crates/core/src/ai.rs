//! The [`AiProvider`] contract + an echo mock.
//!
//! A **designed-but-dormant seam** (see the ROADMAP Phases 15–16 tombstone):
//! the engine contains no LLM code and never reads a model API key — agents
//! (Claude Code, Gemini CLI, Codex) connect from the outside over MCP (ROADMAP
//! Phase 17) and bring their own model. The trait is kept minimal — a completion
//! in, a completion out, plus a capability card — so if in-engine model calls
//! are ever explicitly re-scoped, the contract already exists. Only the `EchoAi`
//! mock implements it.

use async_trait::async_trait;

use crate::error::ProviderResult;
use crate::provider::{Capability, Provider, ProviderInfo, ProviderKind};

/// Who authored a message in a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
}

/// One turn in a conversation.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
}

/// A request for a completion.
#[derive(Debug, Clone, PartialEq)]
pub struct Prompt {
    pub messages: Vec<Message>,
    /// Optional cap on response length in tokens (provider-interpreted).
    pub max_tokens: Option<u32>,
}

impl Prompt {
    /// A single user turn — the common case.
    pub fn of(user: impl Into<String>) -> Self {
        Self {
            messages: vec![Message::user(user)],
            max_tokens: None,
        }
    }
}

/// The provider's response.
#[derive(Debug, Clone, PartialEq)]
pub struct Completion {
    pub text: String,
    /// Model id that produced it, e.g. `"claude-opus-4-8"`.
    pub model: String,
}

/// Anything that can turn a [`Prompt`] into a [`Completion`] — the reasoning
/// backend behind the trading brain, swappable and capability-probed. A raw model
/// (Claude, Gemini, OpenAI) and a coding/agentic assistant (Claude Code, Gemini
/// CLI, Codex) both implement this one seam; a coding agent advertises
/// [`Capability::CodingAgent`] and reports [`ProviderKind::Agent`] so a caller can
/// tell it runs an agentic loop rather than returning a single completion.
///
/// `complete` is `async` because a real backend is a network round-trip (or a
/// spawned agent session); `#[async_trait]` keeps the trait object-safe for a
/// runtime-selected `Box<dyn AiProvider>`.
#[async_trait]
pub trait AiProvider: Provider {
    /// Model (or agent) id this provider will use.
    fn model(&self) -> &str;

    /// Produce a completion for the prompt.
    async fn complete(&self, prompt: &Prompt) -> ProviderResult<Completion>;
}

/// A deterministic, no-network AI provider that echoes the last user message.
/// Lets the orchestration wiring be tested without calling a real model.
#[derive(Debug, Clone)]
pub struct EchoAi {
    id: String,
    model: String,
}

impl EchoAi {
    pub fn new(id: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            model: model.into(),
        }
    }
}

impl Provider for EchoAi {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: self.id.clone(),
            kind: ProviderKind::Ai,
            capabilities: vec![Capability::TextCompletion],
        }
    }
}

#[async_trait]
impl AiProvider for EchoAi {
    fn model(&self) -> &str {
        &self.model
    }

    async fn complete(&self, prompt: &Prompt) -> ProviderResult<Completion> {
        let text = prompt
            .messages
            .iter()
            .rev()
            .find(|m| m.role == Role::User)
            .map(|m| m.content.clone())
            .unwrap_or_default();
        Ok(Completion {
            text,
            model: self.model.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn echo_returns_last_user_message() {
        let ai = EchoAi::new("echo", "echo-1");
        assert_eq!(ai.model(), "echo-1");
        assert!(ai.supports(Capability::TextCompletion));
        assert!(!ai.supports(Capability::ToolUse));

        let out = ai
            .complete(&Prompt::of("is vol cheap on NVDA?"))
            .await
            .unwrap();
        assert_eq!(out.text, "is vol cheap on NVDA?");
        assert_eq!(out.model, "echo-1");
    }
}
