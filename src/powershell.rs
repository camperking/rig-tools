use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use rig::completion::ToolDefinition;
use rig::tool::Tool;
use rust_i18n::t;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{ToolError, format_command_output};

const DEFAULT_TIMEOUT_SECS: u64 = 120;

#[derive(Debug, Deserialize, Serialize)]
pub struct PowerShellArgs {
    pub command: String,
    pub timeout: Option<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct PowerShellTool {
    pub working_dir: Option<Arc<PathBuf>>,
}

impl Tool for PowerShellTool {
    const NAME: &'static str = "powershell";
    type Error = ToolError;
    type Args = PowerShellArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "powershell".to_string(),
            description: t!("powershell.description").to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": t!("powershell.param_command").to_string()
                    },
                    "timeout": {
                        "type": "integer",
                        "description": t!("powershell.param_timeout").to_string()
                    }
                },
                "required": ["command"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let timeout_secs = args.timeout.unwrap_or(DEFAULT_TIMEOUT_SECS);

        let mut cmd = tokio::process::Command::new("powershell");
        cmd.args(["-Command", &args.command]);
        if let Some(wd) = &self.working_dir {
            cmd.current_dir(wd.as_ref());
        }

        let result = tokio::time::timeout(Duration::from_secs(timeout_secs), cmd.output()).await;

        match result {
            Ok(Ok(output)) => Ok(format_command_output(&output)),
            Ok(Err(e)) => Err(ToolError(
                t!("powershell.error_execute_failed", error = e.to_string()).to_string(),
            )),
            Err(_) => Err(ToolError(
                t!("powershell.error_timeout", timeout = timeout_secs).to_string(),
            )),
        }
    }
}
