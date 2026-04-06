use std::path::PathBuf;
use std::sync::Arc;

use rig::completion::ToolDefinition;
use rig::tool::Tool;
use rust_i18n::t;
use serde::{Deserialize, Serialize};
use serde_json::json;
use similar::TextDiff;

use crate::{ToolError, resolve_path};

#[derive(Debug, Deserialize, Serialize)]
pub struct EditOperation {
    pub old_string: String,
    pub new_string: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EditFileArgs {
    pub path: String,
    pub old_string: Option<String>,
    pub new_string: Option<String>,
    pub edits: Option<Vec<EditOperation>>,
}

#[derive(Clone, Debug, Default)]
pub struct EditFile {
    pub working_dir: Option<Arc<PathBuf>>,
}

impl Tool for EditFile {
    const NAME: &'static str = "edit_file";
    type Error = ToolError;
    type Args = EditFileArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "edit_file".to_string(),
            description: t!("edit_file.description").to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": t!("edit_file.param_path").to_string()
                    },
                    "old_string": {
                        "type": "string",
                        "description": t!("edit_file.param_old_string").to_string()
                    },
                    "new_string": {
                        "type": "string",
                        "description": t!("edit_file.param_new_string").to_string()
                    },
                    "edits": {
                        "type": "array",
                        "description": t!("edit_file.param_edits").to_string(),
                        "items": {
                            "type": "object",
                            "properties": {
                                "old_string": {
                                    "type": "string",
                                    "description": t!("edit_file.param_old_string").to_string()
                                },
                                "new_string": {
                                    "type": "string",
                                    "description": t!("edit_file.param_new_string").to_string()
                                }
                            },
                            "required": ["old_string", "new_string"]
                        }
                    }
                },
                "required": ["path"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let path = resolve_path(&args.path, self.working_dir.as_deref());
        let display = path.display().to_string();

        // Determine edit operations
        let operations = match (args.old_string, args.new_string, args.edits) {
            (Some(old), Some(new), None) => vec![EditOperation {
                old_string: old,
                new_string: new,
            }],
            (None, None, Some(edits)) if !edits.is_empty() => edits,
            _ => {
                return Err(ToolError(
                    t!("edit_file.error_missing_args").to_string(),
                ));
            }
        };

        let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
            ToolError(
                t!(
                    "edit_file.error_read_failed",
                    path = &display,
                    error = e.to_string()
                )
                .to_string(),
            )
        })?;

        let original = content.clone();
        let mut current = content;

        for op in &operations {
            if !current.contains(&op.old_string) {
                return Err(ToolError(
                    t!("edit_file.error_not_found", path = &display).to_string(),
                ));
            }
            current = current.replacen(&op.old_string, &op.new_string, 1);
        }

        tokio::fs::write(&path, &current).await.map_err(|e| {
            ToolError(
                t!(
                    "edit_file.error_write_failed",
                    path = &display,
                    error = e.to_string()
                )
                .to_string(),
            )
        })?;

        let diff = TextDiff::from_lines(&original, &current);
        let mut diff_output = String::new();
        for change in diff.iter_all_changes() {
            let sign = match change.tag() {
                similar::ChangeTag::Delete => "-",
                similar::ChangeTag::Insert => "+",
                similar::ChangeTag::Equal => " ",
            };
            diff_output.push_str(sign);
            diff_output.push_str(change.as_str().unwrap_or(""));
        }

        Ok(format!(
            "{}\n\n{}",
            t!("edit_file.success", path = &display),
            diff_output
        ))
    }
}
