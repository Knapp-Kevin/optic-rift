//! Optional Hugging Face credentials. Secrets are never stored in project metadata or frontend storage.
//! Only the native command boundary can read the operating-system credential store.

use keyring::{Entry, Error as KeyringError};
use serde::Serialize;
use std::time::Duration;
use tauri_plugin_opener::OpenerExt;

const SERVICE: &str = "studio.mythologiq.opticrift";
const ACCOUNT: &str = "huggingface-read-token";
const TOKEN_PAGE: &str = "https://huggingface.co/settings/tokens/new?tokenType=read";
const WHOAMI: &str = "https://huggingface.co/api/whoami-v2";

#[derive(Serialize)]
pub struct TokenStatus {
    pub configured: bool,
}

fn entry() -> Result<Entry, String> {
    Entry::new(SERVICE, ACCOUNT)
        .map_err(|_| "Windows Credential Manager is unavailable.".to_string())
}

fn valid_token(raw: &str) -> bool {
    raw.starts_with("hf_")
        && (8..=512).contains(&raw.len())
        && raw.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
}

pub fn status() -> Result<TokenStatus, String> {
    match entry()?.get_password() {
        Ok(secret) => Ok(TokenStatus { configured: valid_token(&secret) }),
        Err(KeyringError::NoEntry) => Ok(TokenStatus { configured: false }),
        Err(_) => Err("Could not access stored Hugging Face credentials.".into()),
    }
}

pub fn save(raw: String) -> Result<TokenStatus, String> {
    let token = raw.trim();
    if !valid_token(token) {
        return Err("Enter a valid Hugging Face access token beginning with hf_.".into());
    }
    entry()?
        .set_password(token)
        .map_err(|_| "Could not securely store the token in Windows Credential Manager.".to_string())?;
    Ok(TokenStatus { configured: true })
}

pub fn remove() -> Result<TokenStatus, String> {
    match entry()?.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => Ok(TokenStatus { configured: false }),
        Err(_) => Err("Could not remove the stored credential.".into()),
    }
}

/// Network access only on an explicit Test click. Never log/return the credential or raw HTTP body.
pub async fn test() -> Result<String, String> {
    let token = match entry()?.get_password() {
        Ok(t) if valid_token(&t) => t,
        Ok(_) => return Err("The saved token is malformed. Replace it in Models.".into()),
        Err(KeyringError::NoEntry) => return Err("Save a token first.".into()),
        Err(_) => return Err("Cannot read the saved token from Windows Credential Manager.".into()),
    };
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Could not prepare a secure verification request.".to_string())?;
    let response = client
        .get(WHOAMI)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|_| "Could not reach Hugging Face over HTTPS. Check your connection.".to_string())?;
    if !response.status().is_success() {
        return match response.status().as_u16() {
            401 | 403 => Err("Hugging Face rejected the token. Check its permissions or regenerate it.".into()),
            _ => Err(format!("Hugging Face verification failed (HTTP {}).", response.status().as_u16())),
        };
    }
    // A successful authenticated whoami response proves account authorization, not model access.
    Ok("Connected. Hugging Face accepted the token.".into())
}

#[tauri::command]
pub fn hf_token_status() -> Result<TokenStatus, String> {
    status()
}

#[tauri::command]
pub async fn hf_token_save(token: String) -> Result<TokenStatus, String> {
    save(token)
}

#[tauri::command]
pub async fn hf_token_remove() -> Result<TokenStatus, String> {
    remove()
}

#[tauri::command]
pub async fn hf_token_test() -> Result<String, String> {
    test().await
}

#[tauri::command]
pub fn hf_open_token_page(app: tauri::AppHandle) -> Result<(), String> {
    // A constant, allowlisted HTTPS URL. Do not accept arbitrary user-supplied URLs.
    app.opener()
        .open_url(TOKEN_PAGE, None::<&str>)
        .map_err(|_| "Could not open your browser. Visit huggingface.co/settings/tokens.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_hugging_face_shaped_credentials_are_accepted() {
        assert!(valid_token("hf_123456789"));
        assert!(!valid_token("ghp_123456789"));
        assert!(!valid_token("hf_not a token"));
        assert!(!valid_token("hf_\nsecret"));
        assert!(!valid_token(""));
    }

    #[test]
    fn onboarding_page_is_hugging_face_read_only() {
        assert!(TOKEN_PAGE.starts_with("https://huggingface.co/settings/tokens/new"));
        assert!(TOKEN_PAGE.contains("tokenType=read"));
        assert_eq!(WHOAMI, "https://huggingface.co/api/whoami-v2");
    }
}
