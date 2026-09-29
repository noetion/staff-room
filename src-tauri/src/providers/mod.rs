use std::path::Path;

mod antigravity;
mod claude;
mod codex;
mod cursor;
mod parse;
mod runtime;

pub const MAX_ARGV_PROMPT_CHARS: usize = 8_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Ask,
    Probe,
    Review,
    QuickEdit,
    Ship,
}

#[derive(Debug, Clone)]
pub struct TurnRequest<'a> {
    pub mode: Mode,
    pub phase: &'a str,
    pub prompt: &'a str,
    pub repository: &'a Path,
    pub session_id: Option<&'a str>,
    pub model: Option<&'a str>,
    pub effort: Option<&'a str>,
    pub final_output_path: &'a Path,
    pub structured_output: bool,
    pub handoff_contract: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedCommand {
    pub args: Vec<String>,
    pub stdin: Option<String>,
    pub assigned_session_id: Option<String>,
}

pub trait ProviderAdapter: Send + Sync {
    fn kind(&self) -> &'static str;
    fn build_command(&self, request: &TurnRequest<'_>) -> Result<PreparedCommand, String>;
}

pub fn build_command(kind: &str, request: &TurnRequest<'_>) -> Result<PreparedCommand, String> {
    // Keep this at command construction too: stale UI/cache state must never
    // enable a provider route whose permission contract is unsupported.
    if kind == "antigravity" || (!is_read_only(request.mode) && kind != "codex") {
        return Err(format!("{kind} has no accepted filesystem write boundary for this route."));
    }
    let adapter: &dyn ProviderAdapter = match kind {
        "codex" => &codex::Adapter,
        "claude" => &claude::Adapter,
        "cursor" => &cursor::Adapter,
        "antigravity" => &antigravity::Adapter,
        _ => return Err(format!("Unsupported provider: {kind}")),
    };
    debug_assert_eq!(adapter.kind(), kind);
    adapter.build_command(request)
}

fn assert_argv_prompt(prompt: &str) -> Result<(), String> {
    if prompt.chars().count() >= MAX_ARGV_PROMPT_CHARS {
        return Err(format!(
            "Provider prompt is {} characters; argv prompts must be under {MAX_ARGV_PROMPT_CHARS} characters.",
            prompt.chars().count()
        ));
    }
    Ok(())
}

fn is_read_only(mode: Mode) -> bool {
    matches!(mode, Mode::Ask | Mode::Probe | Mode::Review)
}

pub(crate) use parse::*;
pub(crate) use runtime::*;
