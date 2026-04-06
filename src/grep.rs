use std::path::PathBuf;
use std::sync::Arc;

use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use regex::RegexBuilder;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use rust_i18n::t;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{ToolError, resolve_path};

const DEFAULT_MAX_RESULTS: usize = 100;
const MAX_LINE_LENGTH: usize = 500;

#[derive(Debug, Deserialize, Serialize)]
pub struct GrepArgs {
    pub pattern: String,
    pub path: Option<String>,
    pub glob: Option<String>,
    pub ignore_case: Option<bool>,
    pub literal: Option<bool>,
    pub context: Option<usize>,
    pub max_results: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct Grep {
    pub working_dir: Option<Arc<PathBuf>>,
}

impl Tool for Grep {
    const NAME: &'static str = "grep";
    type Error = ToolError;
    type Args = GrepArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "grep".to_string(),
            description: t!("grep.description").to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": t!("grep.param_pattern").to_string()
                    },
                    "path": {
                        "type": "string",
                        "description": t!("grep.param_path").to_string()
                    },
                    "glob": {
                        "type": "string",
                        "description": t!("grep.param_glob").to_string()
                    },
                    "ignore_case": {
                        "type": "boolean",
                        "description": t!("grep.param_ignore_case").to_string()
                    },
                    "literal": {
                        "type": "boolean",
                        "description": t!("grep.param_literal").to_string()
                    },
                    "context": {
                        "type": "integer",
                        "description": t!("grep.param_context").to_string()
                    },
                    "max_results": {
                        "type": "integer",
                        "description": t!("grep.param_max_results").to_string()
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

        let ignore_case = args.ignore_case.unwrap_or(false);
        let literal = args.literal.unwrap_or(false);
        let max_results = args.max_results.unwrap_or(DEFAULT_MAX_RESULTS);
        let context_lines = args.context.unwrap_or(0);

        let pattern_str = if literal {
            regex::escape(&args.pattern)
        } else {
            args.pattern.clone()
        };

        let regex = RegexBuilder::new(&pattern_str)
            .case_insensitive(ignore_case)
            .build()
            .map_err(|e| {
                ToolError(
                    t!("grep.error_invalid_pattern", error = e.to_string()).to_string(),
                )
            })?;

        let mut walker = WalkBuilder::new(&search_path);
        walker.hidden(false).git_ignore(true);

        if let Some(glob_pattern) = &args.glob {
            let mut overrides = OverrideBuilder::new(&search_path);
            overrides.add(glob_pattern).map_err(|e| {
                ToolError(
                    t!("grep.error_invalid_pattern", error = e.to_string()).to_string(),
                )
            })?;
            walker.overrides(overrides.build().map_err(|e| {
                ToolError(
                    t!("grep.error_invalid_pattern", error = e.to_string()).to_string(),
                )
            })?);
        }

        let mut matches = Vec::new();
        let mut match_count = 0;

        for entry in walker.build().flatten() {
            if match_count >= max_results {
                break;
            }

            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue, // skip binary/unreadable files
            };

            let lines: Vec<&str> = content.lines().collect();

            for (line_idx, line) in lines.iter().enumerate() {
                if match_count >= max_results {
                    break;
                }

                if regex.is_match(line) {
                    match_count += 1;
                    let rel_path = path
                        .strip_prefix(&search_path)
                        .unwrap_or(path);
                    let line_num = line_idx + 1;

                    if context_lines > 0 {
                        let start = line_idx.saturating_sub(context_lines);
                        let end = (line_idx + context_lines + 1).min(lines.len());

                        for ctx_idx in start..end {
                            let prefix = if ctx_idx == line_idx { ">" } else { " " };
                            let ctx_line = truncate_line(lines[ctx_idx]);
                            matches.push(format!(
                                "{}{}: {}",
                                prefix,
                                ctx_idx + 1,
                                ctx_line
                            ));
                        }
                        matches.push(format!("{}:{}", rel_path.display(), line_num));
                        matches.push("--".to_string());
                    } else {
                        let truncated = truncate_line(line);
                        matches.push(format!(
                            "{}:{}:{}",
                            rel_path.display(),
                            line_num,
                            truncated
                        ));
                    }
                }
            }
        }

        if matches.is_empty() {
            return Ok(t!("grep.no_matches").to_string());
        }

        let mut output = matches.join("\n");
        if match_count >= max_results {
            output.push_str(&format!(
                "\n\n{}",
                t!("grep.results_truncated", max = max_results)
            ));
        }

        Ok(output)
    }
}

fn truncate_line(line: &str) -> &str {
    if line.len() <= MAX_LINE_LENGTH {
        line
    } else {
        &line[..MAX_LINE_LENGTH]
    }
}
