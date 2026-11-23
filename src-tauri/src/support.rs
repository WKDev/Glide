//! Supporter codes and the startup donation prompt.
//!
//! A supporter code is `GLIDE1-<name>.<signature>`, both parts base64url
//! without padding. The signature is Ed25519 over `SIGNING_CONTEXT + name`,
//! made with a private key kept outside this repository
//! (`cargo run --example supporter_code -- issue <key-file> "<name>"`).
//! The app only carries the public key, so codes verify offline.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use tauri_plugin_store::StoreExt;

pub const CODE_PREFIX: &str = "GLIDE1-";
pub const MAX_NAME_CHARS: usize = 40;
const SIGNING_CONTEXT: &[u8] = b"glide-supporter-v1:";
const STORE_KEY: &str = "support";
/// "Not now" hides the startup prompt for this long.
const SNOOZE: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// Public half of the supporter signing key.
const PUBLIC_KEY: [u8; 32] = [
    208, 218, 1, 80, 212, 4, 80, 179, 1, 242, 7, 24, 207, 151, 28, 237, 124, 234, 201, 126, 189,
    27, 155, 212, 126, 0, 218, 97, 245, 74, 56, 160,
];

/// The bytes a supporter code signs.
pub fn signed_message(name: &str) -> Vec<u8> {
    [SIGNING_CONTEXT, name.as_bytes()].concat()
}

/// Build a code from a name and its signature.
pub fn encode_code(name: &str, signature: &[u8; 64]) -> String {
    format!(
        "{CODE_PREFIX}{}.{}",
        URL_SAFE_NO_PAD.encode(name.as_bytes()),
        URL_SAFE_NO_PAD.encode(signature)
    )
}

/// Return the supporter name when `code` carries a valid signature.
pub fn verify_code_with(key: &VerifyingKey, code: &str) -> Option<String> {
    let body = code.trim().strip_prefix(CODE_PREFIX)?;
    let (name, signature) = body.split_once('.')?;
    let name = String::from_utf8(URL_SAFE_NO_PAD.decode(name).ok()?).ok()?;
    let signature: [u8; 64] = URL_SAFE_NO_PAD.decode(signature).ok()?.try_into().ok()?;
    if name.trim().is_empty() || name.chars().count() > MAX_NAME_CHARS {
        return None;
    }
    key.verify(&signed_message(&name), &Signature::from_bytes(&signature))
        .ok()?;
    Some(name)
}

pub fn verify_code(code: &str) -> Option<String> {
    let key = VerifyingKey::from_bytes(&PUBLIC_KEY).ok()?;
    verify_code_with(&key, code)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct SupportState {
    #[serde(default)]
    code: Option<String>,
    /// The user chose a donation link from the prompt; stop asking.
    #[serde(default)]
    decided: bool,
    /// Unix seconds before which the prompt stays hidden.
    #[serde(default)]
    snooze_until: u64,
}

#[derive(Debug, Serialize)]
pub struct SupportStatus {
    supporter: Option<String>,
    prompt: bool,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn load<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> SupportState {
    app.store("config.json")
        .ok()
        .and_then(|store| store.get(STORE_KEY))
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
}

fn save<R: tauri::Runtime>(app: &tauri::AppHandle<R>, state: &SupportState) -> Result<(), String> {
    let store = app.store("config.json").map_err(|e| e.to_string())?;
    store.set(
        STORE_KEY,
        serde_json::to_value(state).map_err(|e| e.to_string())?,
    );
    store.save().map_err(|e| e.to_string())
}

/// Store builds take donations with nothing in return (Store Policies 10.8.2),
/// so supporter codes and their thanks are only honored outside the Store.
const CODES_ENABLED: bool = !crate::STORE_BUILD;

fn status_of(state: &SupportState) -> SupportStatus {
    status_with(state, CODES_ENABLED)
}

fn status_with(state: &SupportState, codes_enabled: bool) -> SupportStatus {
    let supporter = state
        .code
        .as_deref()
        .filter(|_| codes_enabled)
        .and_then(verify_code);
    let prompt = supporter.is_none() && !state.decided && now_secs() >= state.snooze_until;
    SupportStatus { supporter, prompt }
}

/// Whether the startup prompt is due; used to open the window at launch.
pub fn prompt_due<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> bool {
    status_of(&load(app)).prompt
}

#[tauri::command]
pub fn get_support_status(app: tauri::AppHandle) -> SupportStatus {
    status_of(&load(&app))
}

#[tauri::command]
pub fn redeem_supporter_code(app: tauri::AppHandle, code: String) -> Result<String, String> {
    if !CODES_ENABLED {
        return Err("Supporter codes aren't available in the Microsoft Store version.".into());
    }
    let name =
        verify_code(&code).ok_or("This code isn't valid. Check that you copied all of it.")?;
    let mut state = load(&app);
    state.code = Some(code.trim().to_owned());
    save(&app, &state).map_err(|_| "Could not save the code. Try again.".to_owned())?;
    Ok(name)
}

/// `choice` is `"donate"` (a donation link was opened) or `"later"`.
#[tauri::command]
pub fn answer_support_prompt(app: tauri::AppHandle, choice: String) -> Result<(), String> {
    let mut state = load(&app);
    match choice.as_str() {
        "donate" => state.decided = true,
        "later" => state.snooze_until = now_secs() + SNOOZE.as_secs(),
        _ => return Err(format!("unknown choice: {choice}")),
    }
    save(&app, &state)
}

/// Open one of Glide's fixed external links in the default browser.
#[tauri::command]
pub fn open_link(target: String) -> Result<(), String> {
    let url = match target.as_str() {
        "sponsors" => "https://github.com/sponsors/wkdev",
        "coffee" => "https://buymeacoffee.com/wkdev",
        "author" => "https://github.com/wkdev",
        "repo" => "https://github.com/wkdev/glide",
        _ => return Err(format!("unknown link: {target}")),
    };
    open_url(url)
}

#[cfg(target_os = "windows")]
fn open_url(url: &str) -> Result<(), String> {
    use windows::core::HSTRING;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let result = unsafe {
        ShellExecuteW(
            None,
            &HSTRING::from("open"),
            &HSTRING::from(url),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    // ShellExecuteW reports success with a value greater than 32.
    if result.0 as isize > 32 {
        Ok(())
    } else {
        Err("Could not open the browser.".to_owned())
    }
}

#[cfg(not(target_os = "windows"))]
fn open_url(_url: &str) -> Result<(), String> {
    Err("Opening links is only supported on Windows.".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn test_key() -> SigningKey {
        SigningKey::from_bytes(&[7; 32])
    }

    fn issue(key: &SigningKey, name: &str) -> String {
        encode_code(name, &key.sign(&signed_message(name)).to_bytes())
    }

    #[test]
    fn valid_code_returns_name() {
        let key = test_key();
        let code = issue(&key, "Ada Lovelace");
        assert_eq!(
            verify_code_with(&key.verifying_key(), &format!("  {code}\n")),
            Some("Ada Lovelace".to_owned())
        );
    }

    #[test]
    fn tampered_or_foreign_codes_fail() {
        let key = test_key();
        let code = issue(&key, "Ada");
        let forged = code.replace(
            &URL_SAFE_NO_PAD.encode("Ada"),
            &URL_SAFE_NO_PAD.encode("Bob"),
        );
        assert_eq!(verify_code_with(&key.verifying_key(), &forged), None);
        let other = SigningKey::from_bytes(&[9; 32]);
        assert_eq!(verify_code_with(&other.verifying_key(), &code), None);
        assert_eq!(verify_code_with(&key.verifying_key(), "GLIDE1-abc"), None);
        assert_eq!(verify_code_with(&key.verifying_key(), ""), None);
    }

    #[test]
    fn store_builds_ignore_supporter_codes() {
        let key = SigningKey::from_bytes(&[7; 32]);
        let state = SupportState {
            code: Some(issue(&key, "Ada")),
            ..SupportState::default()
        };
        let status = status_with(&state, false);
        assert_eq!(status.supporter, None);
        assert!(status.prompt);
    }

    #[test]
    fn prompt_respects_decision_and_snooze() {
        assert!(status_of(&SupportState::default()).prompt);
        let decided = SupportState {
            decided: true,
            ..SupportState::default()
        };
        assert!(!status_of(&decided).prompt);
        let snoozed = SupportState {
            snooze_until: now_secs() + 60,
            ..SupportState::default()
        };
        assert!(!status_of(&snoozed).prompt);
    }
}
