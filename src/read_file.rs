use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use rust_i18n::t;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{ToolError, resolve_path};

/// Returns the rig-core-supported image MIME type for a path's extension, if any.
fn image_mime_for(path: &Path) -> Option<&'static str> {
    let ext = path.extension().and_then(|e| e.to_str())?.to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "heic" => Some("image/heic"),
        "heif" => Some("image/heif"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}

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

        if let Some(mime) = image_mime_for(&path) {
            let bytes = tokio::fs::read(&path).await.map_err(|e| {
                ToolError(
                    t!("read_file.error_read_failed", path = &display, error = e.to_string())
                        .to_string(),
                )
            })?;
            let data = base64::engine::general_purpose::STANDARD.encode(&bytes);
            return Ok(json!({"type": "image", "data": data, "mimeType": mime}).to_string());
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_tmp(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        p.push(format!("rig-tools-read-{stamp}-{name}"));
        p
    }

    #[test]
    fn image_mime_for_known_extensions() {
        assert_eq!(image_mime_for(Path::new("a.png")), Some("image/png"));
        assert_eq!(image_mime_for(Path::new("a.JPG")), Some("image/jpeg"));
        assert_eq!(image_mime_for(Path::new("a.jpeg")), Some("image/jpeg"));
        assert_eq!(image_mime_for(Path::new("a.svg")), Some("image/svg+xml"));
        assert_eq!(image_mime_for(Path::new("a.txt")), None);
        assert_eq!(image_mime_for(Path::new("noext")), None);
    }

    #[tokio::test]
    async fn reads_image_as_base64_json() {
        // Smallest valid PNG: 1x1 transparent pixel.
        const PNG_BYTES: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
            0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        let path = unique_tmp("pixel.png");
        tokio::fs::write(&path, PNG_BYTES).await.unwrap();

        let tool = ReadFile::default();
        let out = tool
            .call(ReadFileArgs {
                path: path.to_string_lossy().into_owned(),
                start_line: None,
                end_line: None,
            })
            .await
            .unwrap();

        let v: serde_json::Value = serde_json::from_str(&out).expect("output must be JSON");
        assert_eq!(v["type"], "image");
        assert_eq!(v["mimeType"], "image/png");
        let data = v["data"].as_str().expect("data must be a string");
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(data)
            .expect("data must round-trip from base64");
        assert_eq!(decoded.as_slice(), PNG_BYTES);

        let _ = tokio::fs::remove_file(&path).await;
    }

    #[tokio::test]
    async fn reads_text_file_as_plain_text() {
        let path = unique_tmp("hello.txt");
        tokio::fs::write(&path, b"line1\nline2\nline3").await.unwrap();

        let tool = ReadFile::default();
        let out = tool
            .call(ReadFileArgs {
                path: path.to_string_lossy().into_owned(),
                start_line: None,
                end_line: None,
            })
            .await
            .unwrap();

        assert_eq!(out, "line1\nline2\nline3");
        let _ = tokio::fs::remove_file(&path).await;
    }
}
