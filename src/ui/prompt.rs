// ui/prompt.rs — Prompt entry point (PromptContext + render_prompt)

use crate::config::prompt::PromptConfig;
use crate::ui::renderer::render_prompt_modules;

pub struct PromptContext {
    pub last_exit_code:  i32,
    pub cmd_duration_ms: u64,
    pub is_ssh:          bool,
}

pub fn render_prompt(cfg: &PromptConfig, ctx: &PromptContext) -> String {
    render_prompt_modules(cfg, ctx.cmd_duration_ms, ctx.last_exit_code, ctx.is_ssh)
}
