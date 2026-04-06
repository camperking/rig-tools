use std::path::PathBuf;
use std::sync::Arc;

use globset::Glob;
use ignore::WalkBuilder;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use rust_i18n::t;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{ToolError, resolve_path};

const DEFAULT_MAX_RESULTS: usize = 1000;

#[derive(Debug, Deserialize, Serialize)]
pub struct GlobArgs {
    pub pattern: String,
    pub path: Option<String>,
    pub max_results: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct GlobTool {
    pub working_dir: Option<Arc<PathBuf>>,
}

impl Tool for GlobTool {
    const NAME: &'static str = "glob";
    type Error = ToolError;
    type Args = GlobArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "glob".to_string(),
            description: t!("glob.description").to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": t!("glob.param_pattern").to_string()
                    },
                    "path": {
                        "type": "string",
                        "description": t!("glob.param_path").to_string()
                    },
                    "max_results": {
                        "type": "integer",
                        "description": t!("glob.param_max_results").to_string()
                    }
                },
                "required": ["pattern"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let search_path = match &args.path {
            Some(p) => resolve_path(p, self.working_dir.as_deref()),
            None => self
                .working_dir
                .as_ref()
                .map(|wd| wd.as_ref().clone())
                .unwrap_or_else(|| PathBuf::from(".")),
        };

        let max_results = args.max_results.unwrap_or(DEFAULT_MAX_RESULTS);

        let glob = Glob::new(&args.pattern).map_err(|e| {
            ToolError(t!("glob.error_invalid_pattern", error = e.to_string()).to_string())
        })?;
        let matcher = glob.compile_matcher();

        let walker = WalkBuilder::new(&search_path)
            .hidden(false)
            .git_ignore(true)
            .build();

        let mut results: Vec<String> = Vec::new();

        for entry in walker.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let rel_path = path.strip_prefix(&search_path).unwrap_or(path);
            let rel_str = rel_path.to_string_lossy();

            if matcher.is_match(rel_path) || matcher.is_match(rel_str.as_ref()) {
                results.push(rel_str.to_string());
                if results.len() >= max_results {
                    break;
                }
            }
        }

        results.sort();

        if results.is_empty() {
            return Ok(t!("glob.no_matches").to_string());
        }

        let mut output = results.join("\n");
        if results.len() >= max_results {
            output.push_str(&format!(
                "\n\n{}",
                t!("glob.results_truncated", max = max_results)
            ));
        }

        Ok(output)
    }
}
