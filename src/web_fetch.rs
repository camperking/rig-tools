use std::time::Duration;

use base64::Engine;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use rust_i18n::t;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ToolError;

const DEFAULT_TIMEOUT_SECS: u64 = 30;
const MAX_TIMEOUT_SECS: u64 = 120;
const MAX_RESPONSE_BYTES: usize = 5 * 1024 * 1024;
const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
    (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36";
const FALLBACK_UA: &str = "rig-tools";

#[derive(Debug, Default, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WebFetchFormat {
    #[default]
    Markdown,
    Text,
    Html,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WebFetchArgs {
    pub url: String,
    #[serde(default)]
    pub format: Option<WebFetchFormat>,
    pub timeout: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct WebFetch {
    client: reqwest::Client,
}

impl Default for WebFetch {
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl WebFetch {
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }
}

impl Tool for WebFetch {
    const NAME: &'static str = "web_fetch";
    type Error = ToolError;
    type Args = WebFetchArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "web_fetch".to_string(),
            description: t!("web_fetch.description").to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": t!("web_fetch.param_url").to_string()
                    },
                    "format": {
                        "type": "string",
                        "enum": ["markdown", "text", "html"],
                        "description": t!("web_fetch.param_format").to_string()
                    },
                    "timeout": {
                        "type": "integer",
                        "description": t!("web_fetch.param_timeout").to_string()
                    }
                },
                "required": ["url"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        if !args.url.starts_with("http://") && !args.url.starts_with("https://") {
            return Err(ToolError(t!("web_fetch.error_invalid_url").to_string()));
        }

        let format = args.format.unwrap_or_default();
        let timeout_secs = args.timeout.unwrap_or(DEFAULT_TIMEOUT_SECS).min(MAX_TIMEOUT_SECS);
        let deadline = Duration::from_secs(timeout_secs);

        let accept = accept_header_for(format);

        let send = |ua: &'static str| {
            let req = self
                .client
                .get(&args.url)
                .header(reqwest::header::USER_AGENT, ua)
                .header(reqwest::header::ACCEPT, accept)
                .header(reqwest::header::ACCEPT_LANGUAGE, "en-US,en;q=0.9");
            req.send()
        };

        let response = tokio::time::timeout(deadline, send(BROWSER_UA))
            .await
            .map_err(|_| {
                ToolError(
                    t!(
                        "web_fetch.error_timeout",
                        url = &args.url,
                        timeout = timeout_secs
                    )
                    .to_string(),
                )
            })?
            .map_err(|e| {
                ToolError(
                    t!(
                        "web_fetch.error_request_failed",
                        url = &args.url,
                        error = e.to_string()
                    )
                    .to_string(),
                )
            })?;

        let response = if response.status().as_u16() == 403
            && response
                .headers()
                .get("cf-mitigated")
                .and_then(|v| v.to_str().ok())
                == Some("challenge")
        {
            tokio::time::timeout(deadline, send(FALLBACK_UA))
                .await
                .map_err(|_| {
                    ToolError(
                        t!(
                            "web_fetch.error_timeout",
                            url = &args.url,
                            timeout = timeout_secs
                        )
                        .to_string(),
                    )
                })?
                .map_err(|e| {
                    ToolError(
                        t!(
                            "web_fetch.error_request_failed",
                            url = &args.url,
                            error = e.to_string()
                        )
                        .to_string(),
                    )
                })?
        } else {
            response
        };

        if !response.status().is_success() {
            return Err(ToolError(
                t!(
                    "web_fetch.error_status",
                    url = &args.url,
                    status = response.status().as_u16()
                )
                .to_string(),
            ));
        }

        if let Some(len) = response.content_length()
            && len as usize > MAX_RESPONSE_BYTES
        {
            return Err(ToolError(
                t!("web_fetch.error_too_large", url = &args.url).to_string(),
            ));
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let mime = content_type
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();

        let bytes = read_body_capped(response, &args.url).await?;

        if let Some(image_mime) = image_mime_from_content_type(&mime) {
            let data = base64::engine::general_purpose::STANDARD.encode(&bytes);
            return Ok(json!({
                "type": "image",
                "data": data,
                "mimeType": image_mime,
            })
            .to_string());
        }

        let body = String::from_utf8_lossy(&bytes).into_owned();
        let is_html = mime == "text/html" || mime == "application/xhtml+xml";

        let output = match (format, is_html) {
            (WebFetchFormat::Html, _) => body,
            (WebFetchFormat::Markdown, true) => {
                tokio::task::spawn_blocking(move || convert_html_to_markdown(&body))
                    .await
                    .map_err(|e| ToolError(format!("html-to-markdown task failed: {e}")))?
                    .map_err(|e| {
                        ToolError(
                            t!("web_fetch.error_html_to_markdown_failed", error = e).to_string(),
                        )
                    })?
            }
            (WebFetchFormat::Markdown, false) => body,
            (WebFetchFormat::Text, true) => {
                tokio::task::spawn_blocking(move || extract_text_from_html(&body))
                    .await
                    .map_err(|e| ToolError(format!("text-extract task failed: {e}")))?
            }
            (WebFetchFormat::Text, false) => body,
        };

        Ok(format!("{} ({})\n\n{}", args.url, content_type, output))
    }
}

fn accept_header_for(format: WebFetchFormat) -> &'static str {
    match format {
        WebFetchFormat::Markdown => {
            "text/markdown;q=1.0, text/x-markdown;q=0.9, text/plain;q=0.8, text/html;q=0.7, */*;q=0.1"
        }
        WebFetchFormat::Text => {
            "text/plain;q=1.0, text/markdown;q=0.9, text/html;q=0.8, */*;q=0.1"
        }
        WebFetchFormat::Html => {
            "text/html;q=1.0, application/xhtml+xml;q=0.9, text/plain;q=0.8, text/markdown;q=0.7, */*;q=0.1"
        }
    }
}

fn image_mime_from_content_type(mime: &str) -> Option<&'static str> {
    match mime {
        "image/jpeg" | "image/jpg" => Some("image/jpeg"),
        "image/png" => Some("image/png"),
        "image/gif" => Some("image/gif"),
        "image/webp" => Some("image/webp"),
        "image/heic" => Some("image/heic"),
        "image/heif" => Some("image/heif"),
        "image/svg+xml" => Some("image/svg+xml"),
        _ => None,
    }
}

async fn read_body_capped(
    mut response: reqwest::Response,
    url: &str,
) -> Result<Vec<u8>, ToolError> {
    let mut buf: Vec<u8> = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                if buf.len() + chunk.len() > MAX_RESPONSE_BYTES {
                    return Err(ToolError(
                        t!("web_fetch.error_too_large", url = url).to_string(),
                    ));
                }
                buf.extend_from_slice(&chunk);
            }
            Ok(None) => break,
            Err(e) => {
                return Err(ToolError(
                    t!(
                        "web_fetch.error_request_failed",
                        url = url,
                        error = e.to_string()
                    )
                    .to_string(),
                ));
            }
        }
    }
    Ok(buf)
}

fn convert_html_to_markdown(html: &str) -> Result<String, String> {
    let converter = htmd::HtmlToMarkdown::builder()
        .skip_tags(vec!["script", "style", "meta", "link", "noscript"])
        .build();
    converter.convert(html).map_err(|e| e.to_string())
}

fn extract_text_from_html(html: &str) -> String {
    use scraper::{Html, Node};
    const SKIP: &[&str] = &["script", "style", "noscript", "iframe", "object", "embed"];

    fn walk(node: ego_tree::NodeRef<'_, Node>, out: &mut String) {
        for child in node.children() {
            match child.value() {
                Node::Element(el) if SKIP.contains(&el.name()) => continue,
                Node::Text(t) => out.push_str(t),
                _ => walk(child, out),
            }
        }
    }

    let doc = Html::parse_document(html);
    let mut out = String::new();
    walk(doc.tree.root(), &mut out);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rejects_non_http_urls() {
        let tool = WebFetch::default();
        let err = tool
            .call(WebFetchArgs {
                url: "ftp://example.com".to_string(),
                format: None,
                timeout: None,
            })
            .await
            .unwrap_err();
        assert!(err.0.contains("http://") || err.0.contains("https://"));
    }

    #[tokio::test]
    async fn rejects_bare_string_url() {
        let tool = WebFetch::default();
        let err = tool
            .call(WebFetchArgs {
                url: "example.com".to_string(),
                format: None,
                timeout: None,
            })
            .await
            .unwrap_err();
        assert!(!err.0.is_empty());
    }

    #[test]
    fn image_mime_from_content_type_recognises_known_types() {
        assert_eq!(image_mime_from_content_type("image/png"), Some("image/png"));
        assert_eq!(
            image_mime_from_content_type("image/jpeg"),
            Some("image/jpeg")
        );
        assert_eq!(image_mime_from_content_type("image/jpg"), Some("image/jpeg"));
        assert_eq!(
            image_mime_from_content_type("image/svg+xml"),
            Some("image/svg+xml")
        );
        assert_eq!(image_mime_from_content_type("text/html"), None);
        assert_eq!(image_mime_from_content_type(""), None);
    }

    #[test]
    fn accept_header_varies_by_format() {
        assert!(accept_header_for(WebFetchFormat::Markdown).starts_with("text/markdown"));
        assert!(accept_header_for(WebFetchFormat::Text).starts_with("text/plain"));
        assert!(accept_header_for(WebFetchFormat::Html).starts_with("text/html"));
    }

    #[test]
    fn extract_text_strips_script_and_style() {
        let html = r#"<html><head>
            <style>body { color: red; }</style>
            <script>alert('hi')</script>
        </head><body>
            <p>Hello <b>world</b></p>
            <noscript>js disabled</noscript>
        </body></html>"#;
        let text = extract_text_from_html(html);
        assert!(text.contains("Hello"));
        assert!(text.contains("world"));
        assert!(!text.contains("alert"));
        assert!(!text.contains("color: red"));
        assert!(!text.contains("js disabled"));
    }

    #[test]
    fn html_to_markdown_basic_round_trip() {
        let md = convert_html_to_markdown("<h1>Hi</h1><p>A <em>quick</em> test.</p>").unwrap();
        assert!(md.contains("# Hi"));
        assert!(md.contains("quick"));
    }

    #[tokio::test]
    #[ignore]
    async fn fetches_example_com_as_markdown() {
        let tool = WebFetch::default();
        let out = tool
            .call(WebFetchArgs {
                url: "https://example.com".to_string(),
                format: Some(WebFetchFormat::Markdown),
                timeout: Some(15),
            })
            .await
            .unwrap();
        assert!(out.contains("Example Domain"));
    }

    #[tokio::test]
    #[ignore]
    async fn fetches_example_com_as_html() {
        let tool = WebFetch::default();
        let out = tool
            .call(WebFetchArgs {
                url: "https://example.com".to_string(),
                format: Some(WebFetchFormat::Html),
                timeout: Some(15),
            })
            .await
            .unwrap();
        assert!(out.contains("<html") || out.contains("<HTML"));
    }
}
