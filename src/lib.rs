rust_i18n::i18n!("locales", fallback = "en");

use rust_i18n::t;

mod bash;
mod edit_file;
mod glob;
mod grep;
mod ls;
mod powershell;
mod read_file;
mod write_file;

pub use bash::{BashArgs, BashTool};
pub use edit_file::{EditFile, EditFileArgs, EditOperation};
pub use glob::{GlobArgs, GlobTool};
pub use grep::{Grep, GrepArgs};
pub use ls::{Ls, LsArgs};
pub use powershell::{PowerShellArgs, PowerShellTool};
pub use read_file::{ReadFile, ReadFileArgs};
pub use write_file::{WriteFile, WriteFileArgs};

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
