rust_i18n::i18n!("locales", fallback = "en");

use rust_i18n::t;

mod bash;
mod edit_file;
mod glob;
mod grep;
mod ls;
mod powershell;
mod read_file;
mod web_fetch;
mod write_file;

pub use bash::{BashArgs, BashTool};
pub use edit_file::{EditFile, EditFileArgs, EditOperation};
pub use glob::{GlobArgs, GlobTool};
pub use grep::{Grep, GrepArgs};
pub use ls::{Ls, LsArgs};
pub use powershell::{PowerShellArgs, PowerShellTool};
pub use read_file::{ReadFile, ReadFileArgs};
pub use web_fetch::{WebFetch, WebFetchArgs, WebFetchFormat};
pub use write_file::{WriteFile, WriteFileArgs};

/// Metadata for a tool, for use in UI settings panels.
pub struct ToolMeta {
    /// The tool's rig name (e.g. "bash", "read_file").
    pub rig_name: &'static str,
    /// Locale key for the UI-facing tool name.
    name_key: &'static str,
    /// Locale key for a short UI-facing description.
    desc_key: &'static str,
}

impl ToolMeta {
    /// Returns the localized UI name for this tool.
    pub fn name(&self) -> String {
        t!(self.name_key).to_string()
    }

    /// Returns the localized short UI description for this tool.
    pub fn description(&self) -> String {
        t!(self.desc_key).to_string()
    }
}

/// All tools provided by this crate, with their UI metadata.
pub const TOOLS: &[ToolMeta] = &[
    ToolMeta { rig_name: "read_file",  name_key: "ui.read_file.name",  desc_key: "ui.read_file.desc" },
    ToolMeta { rig_name: "edit_file",  name_key: "ui.edit_file.name",  desc_key: "ui.edit_file.desc" },
    ToolMeta { rig_name: "write_file", name_key: "ui.write_file.name", desc_key: "ui.write_file.desc" },
    ToolMeta { rig_name: "bash",       name_key: "ui.bash.name",       desc_key: "ui.bash.desc" },
    ToolMeta { rig_name: "powershell", name_key: "ui.powershell.name", desc_key: "ui.powershell.desc" },
    ToolMeta { rig_name: "grep",       name_key: "ui.grep.name",       desc_key: "ui.grep.desc" },
    ToolMeta { rig_name: "glob",       name_key: "ui.glob.name",       desc_key: "ui.glob.desc" },
    ToolMeta { rig_name: "ls",         name_key: "ui.ls.name",         desc_key: "ui.ls.desc" },
    ToolMeta { rig_name: "web_fetch",  name_key: "ui.web_fetch.name",  desc_key: "ui.web_fetch.desc" },
];

/// Look up UI metadata for a tool by its rig name.
pub fn tool_meta(rig_name: &str) -> Option<&'static ToolMeta> {
    TOOLS.iter().find(|m| m.rig_name == rig_name)
}

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ToolError(pub String);

pub fn resolve_path(path: &str, working_dir: Option<&PathBuf>) -> PathBuf {
    let p = PathBuf::from(path);
    if p.is_absolute() {
        p
    } else if let Some(wd) = working_dir {
        wd.join(p)
    } else {
        p
    }
}

const MAX_OUTPUT_LENGTH: usize = 10000;

pub fn format_command_output(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let mut result = String::new();
    if !stdout.is_empty() {
        result.push_str(&stdout);
    }
    if !stderr.is_empty() {
        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(&t!("command_output.stderr_label"));
        result.push('\n');
        result.push_str(&stderr);
    }

    if result.len() > MAX_OUTPUT_LENGTH {
        // Keep the tail of output (usually more informative)
        let start = result.len() - MAX_OUTPUT_LENGTH;
        let truncated_prefix = t!("command_output.truncated_head").to_string();
        result = format!("{}\n{}", truncated_prefix, &result[start..]);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_path_absolute() {
        let result = resolve_path("/tmp/foo", None);
        assert_eq!(result, PathBuf::from("/tmp/foo"));
    }

    #[test]
    fn resolve_path_relative_with_wd() {
        let wd = PathBuf::from("/tmp");
        let result = resolve_path("foo", Some(&wd));
        assert_eq!(result, PathBuf::from("/tmp/foo"));
    }

    #[test]
    fn resolve_path_relative_no_wd() {
        let result = resolve_path("foo", None);
        assert_eq!(result, PathBuf::from("foo"));
    }

    #[test]
    fn format_output_stdout_only() {
        let output = std::process::Output {
            status: std::process::ExitStatus::default(),
            stdout: b"hello".to_vec(),
            stderr: vec![],
        };
        assert_eq!(format_command_output(&output), "hello");
    }

    #[test]
    fn format_output_both() {
        let output = std::process::Output {
            status: std::process::ExitStatus::default(),
            stdout: b"out".to_vec(),
            stderr: b"err".to_vec(),
        };
        assert_eq!(format_command_output(&output), "out\nstderr:\nerr");
    }

    #[test]
    fn format_output_truncation() {
        let output = std::process::Output {
            status: std::process::ExitStatus::default(),
            stdout: vec![b'a'; 11000],
            stderr: vec![],
        };
        let result = format_command_output(&output);
        assert!(result.len() < 11000);
        assert!(result.starts_with("... (beginning of output truncated)"));
    }
}
