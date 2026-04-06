use std::path::PathBuf;
use std::sync::Arc;

use rig::completion::ToolDefinition;
use rig::tool::Tool;
use rust_i18n::t;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{ToolError, resolve_path};

const DEFAULT_MAX_RESULTS: usize = 500;

#[derive(Debug, Deserialize, Serialize)]
pub struct LsArgs {
    pub path: Option<String>,
    pub max_results: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct Ls {
    pub working_dir: Option<Arc<PathBuf>>,
}

impl Tool for Ls {
    const NAME: &'static str = "ls";
    type Error = ToolError;
    type Args = LsArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "ls".to_string(),
            description: t!("ls.description").to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": t!("ls.param_path").to_string()
                    },
                    "max_results": {
                        "type": "integer",
                        "description": t!("ls.param_max_results").to_string()
                    }
                }
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let dir_path = match &args.path {
            Some(p) => resolve_path(p, self.working_dir.as_deref()),
            None => self
                .working_dir
                .as_ref()
                .map(|wd| wd.as_ref().clone())
                .unwrap_or_else(|| PathBuf::from(".")),
        };

        let max_results = args.max_results.unwrap_or(DEFAULT_MAX_RESULTS);

        let mut read_dir = tokio::fs::read_dir(&dir_path).await.map_err(|e| {
            ToolError(
                t!(
                    "ls.error_read_failed",
                    path = dir_path.display().to_string(),
                    error = e.to_string()
                )
                .to_string(),
            )
        })?;

        let mut entries: Vec<String> = Vec::new();

        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry
                .file_type()
                .await
                .map(|ft| ft.is_dir())
                .unwrap_or(false);

            if is_dir {
                entries.push(format!("{}/", name));
            } else {
                entries.push(name);
            }
        }

        if entries.is_empty() {
            return Ok(t!("ls.empty_directory").to_string());
        }

        entries.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));

        let truncated = entries.len() > max_results;
        entries.truncate(max_results);

        let mut output = entries.join("\n");
        if truncated {
            output.push_str(&format!(
                "\n\n{}",
                t!("ls.results_truncated", max = max_results)
            ));
        }

        Ok(output)
    }
}
