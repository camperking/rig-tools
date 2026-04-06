use std::path::PathBuf;
use std::sync::Arc;

use rig::completion::ToolDefinition;
use rig::tool::Tool;
use rust_i18n::t;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{ToolError, resolve_path};

#[derive(Debug, Deserialize, Serialize)]
pub struct WriteFileArgs {
    pub path: String,
    pub content: String,
}

#[derive(Clone, Debug, Default)]
pub struct WriteFile {
    pub working_dir: Option<Arc<PathBuf>>,
}

impl Tool for WriteFile {
    const NAME: &'static str = "write_file";
    type Error = ToolError;
    type Args = WriteFileArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "write_file".to_string(),
            description: t!("write_file.description").to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": t!("write_file.param_path").to_string()
                    },
                    "content": {
                        "type": "string",
                        "description": t!("write_file.param_content").to_string()
                    }
                },
                "required": ["path", "content"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let path = resolve_path(&args.path, self.working_dir.as_deref());
        let display = path.display().to_string();

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                ToolError(t!("write_file.error_create_dirs", error = e.to_string()).to_string())
            })?;
        }

        tokio::fs::write(&path, &args.content).await.map_err(|e| {
            ToolError(t!("write_file.error_write_failed", path = &display, error = e.to_string()).to_string())
        })?;

        Ok(t!("write_file.success", path = &display).to_string())
    }
}
