//! Optional Hugging Face credentials. Secrets are never stored in project metadata or frontend storage.
//! Only the native command boundary can read the operating-system credential store, and no command
//! returns the token text.

use keyring::{Entry, Error as KeyringError};
use serde::Serialize;
use std::time::Duration;
use tauri_plugin_opener::OpenerExt;

const SERVICE: &str = "studio.mythologiq.opticrift";
const ACCOUNT: &str = "huggingface-read-token";
const TOKEN_PAGE: &str = "https://huggingface.co/settings/tokens/new?tokenType=read";
const WHOAMI: &str = "https://huggingface.co/api/whoami-v2";
const VERIFY_TIMEOUT: Duration = Duration::from_secs(12);

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct TokenStatus {
    pub configured: bool,
}

/// One credential slot in Windows Credential Manager. Tests use their own slot so they never
/// read, overwrite or delete the user's real token.
#[derive(Clone, Copy)]
pub(crate) struct CredentialSlot {
    service: &'static str,
    account: &'static str,
}

pub(crate) const USER_SLOT: CredentialSlot = CredentialSlot {
    service: SERVICE,
    account: ACCOUNT,
};

impl CredentialSlot {
    fn entry(self) -> Result<Entry, String> {
        Entry::new(self.service, self.account)
            .map_err(|_| "Windows Credential Manager is unavailable.".to_string())
    }

    pub(crate) fn status(self) -> Result<TokenStatus, String> {
        match self.entry()?.get_password() {
            Ok(secret) => Ok(TokenStatus {
                configured: valid_token(&secret),
            }),
            Err(KeyringError::NoEntry) => Ok(TokenStatus { configured: false }),
            Err(_) => Err("Could not access stored Hugging Face credentials.".into()),
        }
    }

    pub(crate) fn save(self, raw: &str) -> Result<TokenStatus, String> {
        let token = raw.trim();
        if !valid_token(token) {
            return Err("Enter a valid Hugging Face access token beginning with hf_.".into());
        }
        self.entry()?.set_password(token).map_err(|_| {
            "Could not securely store the token in Windows Credential Manager.".to_string()
        })?;
        Ok(TokenStatus { configured: true })
    }

    pub(crate) fn remove(self) -> Result<TokenStatus, String> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(TokenStatus { configured: false }),
            Err(_) => Err("Could not remove the stored credential.".into()),
        }
    }

    /// The stored token, for native HTTP requests only. Never crosses IPC.
    pub(crate) fn token(self) -> Result<Option<String>, String> {
        match self.entry()?.get_password() {
            Ok(t) if valid_token(&t) => Ok(Some(t)),
            Ok(_) => Err("The saved token is malformed. Replace it in Models.".into()),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(_) => Err("Cannot read the saved token from Windows Credential Manager.".into()),
        }
    }
}

fn valid_token(raw: &str) -> bool {
    raw.starts_with("hf_")
        && (8..=512).contains(&raw.len())
        && raw.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
}

/// Verify a token against a whoami endpoint. Errors never contain the token or response body.
async fn verify(url: &str, token: &str, timeout: Duration) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Could not prepare a secure verification request.".to_string())?;
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                "Hugging Face did not respond in time. Try again later.".to_string()
            } else {
                "Could not reach Hugging Face over HTTPS. Check your connection.".to_string()
            }
        })?;
    let code = response.status().as_u16();
    match code {
        200..=299 => Ok("Connected. Hugging Face accepted the token.".into()),
        401 | 403 => Err(
            "Hugging Face rejected the token. It may be invalid, expired or revoked; check its permissions or create a new one."
                .into(),
        ),
        300..=399 => Err(format!(
            "Hugging Face verification was redirected (HTTP {code}); not followed."
        )),
        429 => Err("Hugging Face is rate limiting requests. Try again shortly.".into()),
        500..=599 => Err(format!(
            "Hugging Face is unavailable right now (HTTP {code}). Try again later."
        )),
        _ => Err(format!("Hugging Face verification failed (HTTP {code}).")),
    }
}

/// Network access only on an explicit Test click.
pub async fn test(slot: CredentialSlot) -> Result<String, String> {
    match slot.token()? {
        Some(token) => verify(WHOAMI, &token, VERIFY_TIMEOUT).await,
        None => Err("Save a token first.".into()),
    }
}

#[tauri::command]
pub fn hf_token_status() -> Result<TokenStatus, String> {
    USER_SLOT.status()
}

#[tauri::command]
pub async fn hf_token_save(token: String) -> Result<TokenStatus, String> {
    USER_SLOT.save(&token)
}

#[tauri::command]
pub async fn hf_token_remove() -> Result<TokenStatus, String> {
    USER_SLOT.remove()
}

#[tauri::command]
pub async fn hf_token_test() -> Result<String, String> {
    test(USER_SLOT).await
}

#[tauri::command]
pub fn hf_open_token_page<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    // A constant, allowlisted HTTPS URL. Do not accept arbitrary user-supplied URLs.
    app.opener()
        .open_url(TOKEN_PAGE, None::<&str>)
        .map_err(|_| {
            "Could not open your browser. Visit huggingface.co/settings/tokens.".to_string()
        })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    pub(crate) const TEST_SLOT: CredentialSlot = CredentialSlot {
        service: "studio.mythologiq.opticrift.test",
        account: "huggingface-read-token-unit-test",
    };
    const FAKE: &str = "hf_OpticRiftUnitTestNotARealToken123";

    #[test]
    fn only_hugging_face_shaped_credentials_are_accepted() {
        assert!(valid_token("hf_123456789"));
        assert!(!valid_token("ghp_123456789"));
        assert!(!valid_token("hf_not a token"));
        assert!(!valid_token("hf_\nsecret"));
        assert!(!valid_token(""));
        assert!(!valid_token(&format!("hf_{}", "a".repeat(600))));
    }

    #[test]
    fn onboarding_page_is_hugging_face_read_only() {
        assert!(TOKEN_PAGE.starts_with("https://huggingface.co/settings/tokens/new"));
        assert!(TOKEN_PAGE.contains("tokenType=read"));
        assert_eq!(WHOAMI, "https://huggingface.co/api/whoami-v2");
    }

    /// Real Windows Credential Manager round trip in an isolated test slot.
    #[test]
    fn credential_manager_save_status_read_remove() {
        let slot = TEST_SLOT;
        slot.remove().unwrap();
        assert_eq!(slot.status().unwrap(), TokenStatus { configured: false });
        assert!(slot.save("not-a-token").is_err());
        assert_eq!(slot.status().unwrap(), TokenStatus { configured: false });
        assert_eq!(
            slot.save(&format!("  {FAKE}  ")).unwrap(),
            TokenStatus { configured: true }
        );
        assert_eq!(slot.status().unwrap(), TokenStatus { configured: true });
        assert_eq!(slot.token().unwrap().as_deref(), Some(FAKE), "trimmed");
        // The status payload that crosses IPC carries no secret.
        let json = serde_json::to_string(&slot.status().unwrap()).unwrap();
        assert_eq!(json, r#"{"configured":true}"#);
        assert_eq!(slot.remove().unwrap(), TokenStatus { configured: false });
        assert_eq!(
            slot.remove().unwrap(),
            TokenStatus { configured: false },
            "idempotent"
        );
        assert_eq!(slot.token().unwrap(), None);
    }

    /// One-shot HTTP server returning `response`; `None` accepts and never answers.
    fn serve_once(response: Option<&'static str>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                match response {
                    Some(r) => {
                        let _ = stream.write_all(r.as_bytes());
                    }
                    None => thread::sleep(Duration::from_secs(5)),
                }
            }
        });
        format!("http://{addr}/api/whoami-v2")
    }

    fn run(url: &str, timeout: Duration) -> Result<String, String> {
        tauri::async_runtime::block_on(verify(url, FAKE, timeout))
    }

    #[test]
    fn verification_outcomes_are_distinct_and_never_leak_the_token() {
        let cases = [
            (
                "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}",
                true,
                "Connected",
            ),
            (
                "HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n",
                false,
                "rejected",
            ),
            (
                "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n",
                false,
                "rejected",
            ),
            (
                "HTTP/1.1 302 Found\r\nLocation: http://evil.example/\r\nContent-Length: 0\r\n\r\n",
                false,
                "redirected",
            ),
            (
                "HTTP/1.1 429 Too Many Requests\r\nContent-Length: 0\r\n\r\n",
                false,
                "rate limiting",
            ),
            (
                "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n",
                false,
                "unavailable",
            ),
        ];
        for (response, ok, needle) in cases {
            let result = run(&serve_once(Some(response)), Duration::from_secs(5));
            let text = match &result {
                Ok(t) | Err(t) => t.clone(),
            };
            assert_eq!(result.is_ok(), ok, "{response:?} -> {text}");
            assert!(text.contains(needle), "{response:?} -> {text}");
            assert!(!text.contains(FAKE), "token leaked: {text}");
        }
    }

    #[test]
    fn timeout_and_unreachable_are_reported_without_the_token() {
        let timed_out = run(&serve_once(None), Duration::from_millis(500)).unwrap_err();
        assert!(timed_out.contains("did not respond in time"), "{timed_out}");
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let closed = format!("http://{}/", listener.local_addr().unwrap());
        drop(listener);
        // Windows retries a refused SYN for about 2 s, so allow longer than that.
        let refused = run(&closed, Duration::from_secs(10)).unwrap_err();
        assert!(refused.contains("Could not reach"), "{refused}");
        assert!(!timed_out.contains(FAKE) && !refused.contains(FAKE));
    }
}
