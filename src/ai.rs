use crate::config::AiConfig;
use anyhow::{Context, Result};
use gtk::{glib, Label};
use std::{thread, time::Duration};

pub(crate) fn ask(ai: &AiConfig, prompt: &str, output: &Label) {
    let (tx, rx) = async_channel::bounded::<String>(1);
    let ai = ai.clone();
    let prompt = prompt.to_string();
    thread::spawn(move || {
        let message = match request_completion(&ai, &prompt) {
            Ok(text) => text,
            Err(error) => format!("AI request failed: {error:#}"),
        };
        let _ = tx.send_blocking(message);
    });

    let output = output.clone();
    glib::spawn_future_local(async move {
        if let Ok(message) = rx.recv().await {
            output.set_text(&message);
        }
    });
}

fn request_completion(ai: &AiConfig, prompt: &str) -> Result<String> {
    let url = format!("{}/chat/completions", ai.base_url.trim_end_matches('/'));
    let mut request = ureq::post(&url)
        .timeout(Duration::from_secs(60))
        .set("Content-Type", "application/json");
    if let Some(key) = ai.resolve_api_key() {
        request = request.set("Authorization", &format!("Bearer {key}"));
    }

    let response = request.send_json(serde_json::json!({
        "model": ai.model,
        "messages": [{"role": "user", "content": prompt}],
    }));

    let body: serde_json::Value = match response {
        Ok(response) => response.into_json().context("read response body")?,
        Err(ureq::Error::Status(code, response)) => {
            let mut detail = response.into_string().unwrap_or_default();
            detail.truncate(500);
            anyhow::bail!("HTTP {code} from {url}: {detail}");
        }
        Err(error) => return Err(error).with_context(|| format!("request {url}")),
    };

    body["choices"][0]["message"]["content"]
        .as_str()
        .map(|text| text.trim().to_string())
        .context("no completion text in response")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };

    #[test]
    fn request_completion_talks_to_openai_compatible_endpoint() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buf = [0u8; 4096];
            loop {
                let n = stream.read(&mut buf).unwrap();
                request.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&request);
                if let Some(headers_end) = text.find("\r\n\r\n") {
                    let content_length = text
                        .lines()
                        .find_map(|line| {
                            line.to_lowercase()
                                .strip_prefix("content-length:")
                                .map(str::trim)
                                .map(String::from)
                        })
                        .and_then(|value| value.parse::<usize>().ok())
                        .unwrap_or(0);
                    if request.len() >= headers_end + 4 + content_length {
                        break;
                    }
                }
            }
            let body =
                r#"{"choices":[{"message":{"role":"assistant","content":"Hello from mock"}}]}"#;
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(reply.as_bytes()).unwrap();
            String::from_utf8_lossy(&request).to_string()
        });

        let ai = AiConfig {
            base_url: format!("http://{addr}/v1"),
            api_key: "test-key".to_string(),
            model: "test-model".to_string(),
        };
        let text = request_completion(&ai, "hi there").unwrap();
        assert_eq!(text, "Hello from mock");

        let request = server.join().unwrap();
        assert!(
            request.starts_with("POST /v1/chat/completions"),
            "{request}"
        );
        assert!(request.contains("Bearer test-key"), "{request}");
        assert!(request.contains("test-model"), "{request}");
        assert!(request.contains("hi there"), "{request}");
    }
}
