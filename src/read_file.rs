use std::path::PathBuf;
use std::sync::Arc;

use rig::completion::ToolDefinition;
use rig::tool::Tool;
use rust_i18n::t;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{ToolError, resolve_path};

/// Default maximum number of lines returned when no range is specified.
const DEFAULT_MAX_LINES: usize = 500;

#[derive(Debug, Deserialize, Serialize)]
pub struct ReadFileArgs {
    pub path: String,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct ReadFile {
    pub working_dir: Option<Arc<PathBuf>>,
    /// Maximum lines returned when no range is specified. Defaults to 500.
    pub max_lines: usize,
}

impl Default for ReadFile {
    fn default() -> Self {
        Self {
            working_dir: None,
            max_lines: DEFAULT_MAX_LINES,
        }
    }
}

impl Tool for ReadFile {
    const NAME: &'static str = "read_file";
    type Error = ToolError;
    type Args = ReadFileArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "read_file".to_string(),
            description: t!("read_file.description").to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": t!("read_file.param_path").to_string()
                    },
                    "start_line": {
                        "type": "integer",
                        "description": t!("read_file.param_start_line").to_string()
                    },
                    "end_line": {
                        "type": "integer",
                        "description": t!("read_file.param_end_line").to_string()
                    }
                },
                "required": ["path"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let path = resolve_path(&args.path, self.working_dir.as_deref());
        let display = path.display().to_string();
        let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
            ToolError(t!("read_file.error_read_failed", path = &display, error = e.to_string()).to_string())
        })?;

        let all_lines: Vec<&str> = content.lines().collect();
        let total_lines = all_lines.len();

        let start = args.start_line.unwrap_or(1).max(1);
        let end = args
            .end_line
            .unwrap_or(start.saturating_add(self.max_lines) - 1)
            .min(total_lines);

        if start > total_lines {
            return Err(ToolError(
                t!("read_file.error_start_beyond_eof", start = start, total = total_lines)
                    .to_string(),
            ));
        }

        let selected: Vec<&str> = all_lines[(start - 1)..end].to_vec();
        let shown = selected.len();
        let mut output = selected.join("\n");

        if shown < total_lines {
            output.push_str(&format!(
                "\n\n{}",
                t!("read_file.showing_lines", start = start, end = end, total = total_lines)
            ));
        }

        Ok(output)
    }
}
