use super::types::OAuthError;
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

/// Parameters extracted from the OAuth callback.
#[derive(Debug, Clone)]
pub struct CallbackParams {
    pub code: String,
    pub state: String,
}

/// Start a temporary localhost HTTP server for the OAuth callback.
/// Returns the port and a channel that will receive the callback parameters.
pub async fn start_callback_server() -> Result<(u16, oneshot::Receiver<CallbackParams>), OAuthError> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| OAuthError::Internal(format!("Failed to bind callback server: {}", e)))?;

    let port = listener
        .local_addr()
        .map_err(|e| OAuthError::Internal(format!("Failed to get local addr: {}", e)))?
        .port();

    let (tx, rx) = oneshot::channel();

    tokio::spawn(async move {
        let result = tokio::time::timeout(std::time::Duration::from_secs(300), async {
            let (mut stream, _) = listener
                .accept()
                .await
                .map_err(|e| OAuthError::Internal(format!("Accept failed: {}", e)))?;

            let mut buf = vec![0u8; 4096];
            let n = stream
                .read(&mut buf)
                .await
                .map_err(|e| OAuthError::Internal(format!("Read failed: {}", e)))?;

            let request = String::from_utf8_lossy(&buf[..n]);

            // Parse the first line: "GET /callback?code=xxx&state=yyy HTTP/1.1"
            let params = parse_callback_request(&request)?;

            // Send success HTML response
            let html = r#"<!DOCTYPE html>
<html><head><title>Brightwing</title>
<style>body{font-family:system-ui;display:flex;justify-content:center;align-items:center;height:100vh;margin:0;background:#1a1a2e;color:#e0e0e0}
.card{text-align:center;padding:2em;border-radius:12px;background:#16213e;border:1px solid #0f3460}
h1{color:#4ecca3;margin-bottom:0.5em}p{color:#a0a0a0}</style></head>
<body><div class="card"><h1>Authenticated</h1><p>You can close this tab and return to Brightwing.</p></div></body></html>"#;

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                html.len(),
                html
            );

            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.shutdown().await;

            Ok::<CallbackParams, OAuthError>(params)
        })
        .await;

        match result {
            Ok(Ok(params)) => {
                let _ = tx.send(params);
            }
            Ok(Err(_)) | Err(_) => {
                // Timeout or error — channel will be dropped, receiver gets error
            }
        }
    });

    Ok((port, rx))
}

fn parse_callback_request(request: &str) -> Result<CallbackParams, OAuthError> {
    let first_line = request
        .lines()
        .next()
        .ok_or_else(|| OAuthError::Internal("Empty request".to_string()))?;

    // Extract path from "GET /callback?... HTTP/1.1"
    let path = first_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| OAuthError::Internal("Malformed request line".to_string()))?;

    let query = path
        .split_once('?')
        .map(|(_, q)| q)
        .ok_or_else(|| OAuthError::Internal("No query string in callback".to_string()))?;

    let params: HashMap<String, String> = query
        .split('&')
        .filter_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            Some((
                urlencoding::decode(k).ok()?.to_string(),
                urlencoding::decode(v).ok()?.to_string(),
            ))
        })
        .collect();

    // Check for error response
    if let Some(error) = params.get("error") {
        let desc = params
            .get("error_description")
            .cloned()
            .unwrap_or_default();
        return Err(OAuthError::ExchangeFailed(format!("{}: {}", error, desc)));
    }

    let code = params
        .get("code")
        .cloned()
        .ok_or_else(|| OAuthError::Internal("Missing 'code' parameter".to_string()))?;

    let state = params
        .get("state")
        .cloned()
        .ok_or_else(|| OAuthError::Internal("Missing 'state' parameter".to_string()))?;

    Ok(CallbackParams { code, state })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_callback_request() {
        let request = "GET /callback?code=abc123&state=xyz789 HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let params = parse_callback_request(request).unwrap();
        assert_eq!(params.code, "abc123");
        assert_eq!(params.state, "xyz789");
    }

    #[test]
    fn test_parse_callback_url_encoded() {
        let request = "GET /callback?code=abc%20123&state=xyz%3D789 HTTP/1.1\r\n\r\n";
        let params = parse_callback_request(request).unwrap();
        assert_eq!(params.code, "abc 123");
        assert_eq!(params.state, "xyz=789");
    }

    #[test]
    fn test_parse_callback_error() {
        let request =
            "GET /callback?error=access_denied&error_description=User+denied HTTP/1.1\r\n\r\n";
        let result = parse_callback_request(request);
        assert!(matches!(result, Err(OAuthError::ExchangeFailed(_))));
    }

    #[test]
    fn test_parse_callback_missing_code() {
        let request = "GET /callback?state=xyz HTTP/1.1\r\n\r\n";
        let result = parse_callback_request(request);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_callback_server_receives_params() {
        let (port, rx) = start_callback_server().await.unwrap();

        // Simulate browser redirect
        let url = format!(
            "http://127.0.0.1:{}/callback?code=test-code&state=test-state",
            port
        );
        let _ = reqwest::get(&url).await;

        let params = rx.await.unwrap();
        assert_eq!(params.code, "test-code");
        assert_eq!(params.state, "test-state");
    }
}
