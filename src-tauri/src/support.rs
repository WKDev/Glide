//! Supporter codes and the startup donation prompt.
//!
//! A supporter code is `GLIDE2-<name>.<signature>`, both parts base64url
//! without padding. The signature is Ed25519 over
//! `SIGNING_CONTEXT + name + "\n" + sha256(email)`, so a code only works
//! together with the supporter's email: sharing a code publicly means sharing
//! that email too. Codes are made with a private key kept outside this
//! repository (`cargo run --example supporter_code -- issue ...`). The app
//! carries only the public key and verifies offline; it stores the email's
//! hash, never the address.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri_plugin_store::StoreExt;

pub const CODE_PREFIX: &str = "GLIDE2-";
pub const MAX_NAME_CHARS: usize = 40;
const SIGNING_CONTEXT: &[u8] = b"glide-supporter-v2:";
/// Characters of the signature used as a code's public ID.
const CODE_ID_CHARS: usize = 12;
const STORE_KEY: &str = "support";
/// "Not now" hides the startup prompt for this long.
const SNOOZE: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// Public half of the supporter signing key.
const PUBLIC_KEY: [u8; 32] = [
    208, 218, 1, 80, 212, 4, 80, 179, 1, 242, 7, 24, 207, 151, 28, 237, 124, 234, 201, 126, 189,
    27, 155, 212, 126, 0, 218, 97, 245, 74, 56, 160,
];

/// IDs of codes found shared publicly (see `supporters.csv` beside the key).
/// A listed code stops working once users install the update carrying it.
const REVOKED_CODE_IDS: &[&str] = &[];

/// Hex SHA-256 of the email, trimmed and lowercased.
pub fn email_hash(email: &str) -> String {
    Sha256::digest(email.trim().to_lowercase().as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Loose shape check so typos fail before signing or verifying.
pub fn looks_like_email(email: &str) -> bool {
    let email = email.trim();
    email
        .split_once('@')
        .is_some_and(|(user, domain)| !user.is_empty() && domain.contains('.'))
        && !email.contains(char::is_whitespace)
}

/// The bytes a supporter code signs.
pub fn signed_message(name: &str, email_hash: &str) -> Vec<u8> {
    [
        SIGNING_CONTEXT,
        name.as_bytes(),
        b"\n",
        email_hash.as_bytes(),
    ]
    .concat()
}

/// Build a code from a name and its signature.
pub fn encode_code(name: &str, signature: &[u8; 64]) -> String {
    format!(
        "{CODE_PREFIX}{}.{}",
        URL_SAFE_NO_PAD.encode(name.as_bytes()),
        URL_SAFE_NO_PAD.encode(signature)
    )
}

/// Short public ID of a code, recorded at issue time and used to revoke it.
pub fn code_id(code: &str) -> Option<String> {
    let (_, signature) = code.trim().strip_prefix(CODE_PREFIX)?.split_once('.')?;
    signature.get(..CODE_ID_CHARS).map(str::to_owned)
}

/// Return the supporter name when `code` is validly signed for this email hash.
pub fn verify_code_with(key: &VerifyingKey, code: &str, email_hash: &str) -> Option<String> {
    let (name, signature) = code.trim().strip_prefix(CODE_PREFIX)?.split_once('.')?;
    let name = String::from_utf8(URL_SAFE_NO_PAD.decode(name).ok()?).ok()?;
    let signature: [u8; 64] = URL_SAFE_NO_PAD.decode(signature).ok()?.try_into().ok()?;
    if name.trim().is_empty() || name.chars().count() > MAX_NAME_CHARS {
        return None;
    }
    key.verify(
        &signed_message(&name, email_hash),
        &Signature::from_bytes(&signature),
    )
    .ok()?;
    Some(name)
}

/// Verify against Glide's key and the revocation list.
pub fn verify_code(code: &str, email_hash: &str) -> Option<String> {
    if code_id(code).is_some_and(|id| REVOKED_CODE_IDS.contains(&id.as_str())) {
        return None;
    }
    let key = VerifyingKey::from_bytes(&PUBLIC_KEY).ok()?;
    verify_code_with(&key, code, email_hash)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct SupportState {
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    email_hash: Option<String>,
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
    let supporter = match (&state.code, &state.email_hash) {
        (Some(code), Some(hash)) if codes_enabled => verify_code(code, hash),
        _ => None,
    };
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
pub fn redeem_supporter_code(
    app: tauri::AppHandle,
    code: String,
    email: String,
) -> Result<String, String> {
    if !CODES_ENABLED {
        return Err("Supporter codes aren't available in the Microsoft Store version.".into());
    }
    if !looks_like_email(&email) {
        return Err("Enter the email address the code was issued to.".into());
    }
    let hash = email_hash(&email);
    let name = verify_code(&code, &hash).ok_or(
        "This code doesn't match that email. Check both against the message you received.",
    )?;
    let mut state = load(&app);
    state.code = Some(code.trim().to_owned());
    state.email_hash = Some(hash);
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

    const EMAIL: &str = "ada@example.com";

    fn test_key() -> SigningKey {
        SigningKey::from_bytes(&[7; 32])
    }

    fn issue(key: &SigningKey, name: &str, email: &str) -> String {
        let signature = key.sign(&signed_message(name, &email_hash(email)));
        encode_code(name, &signature.to_bytes())
    }

    #[test]
    fn valid_code_returns_name_for_its_email() {
        let key = test_key();
        let code = issue(&key, "Ada Lovelace", EMAIL);
        // Email matching ignores case and surrounding space.
        assert_eq!(
            verify_code_with(
                &key.verifying_key(),
                &format!("  {code}\n"),
                &email_hash(" ADA@Example.com ")
            ),
            Some("Ada Lovelace".to_owned())
        );
    }

    #[test]
    fn code_fails_with_another_email() {
        let key = test_key();
        let code = issue(&key, "Ada", EMAIL);
        assert_eq!(
            verify_code_with(&key.verifying_key(), &code, &email_hash("bob@example.com")),
            None
        );
    }

    #[test]
    fn tampered_or_foreign_codes_fail() {
        let key = test_key();
        let code = issue(&key, "Ada", EMAIL);
        let hash = email_hash(EMAIL);
        let forged = code.replace(
            &URL_SAFE_NO_PAD.encode("Ada"),
            &URL_SAFE_NO_PAD.encode("Bob"),
        );
        assert_eq!(verify_code_with(&key.verifying_key(), &forged, &hash), None);
        let other = SigningKey::from_bytes(&[9; 32]);
        assert_eq!(verify_code_with(&other.verifying_key(), &code, &hash), None);
        assert_eq!(
            verify_code_with(&key.verifying_key(), "GLIDE2-abc", &hash),
            None
        );
        assert_eq!(verify_code_with(&key.verifying_key(), "", &hash), None);
    }

    #[test]
    fn code_id_is_a_stable_signature_prefix() {
        let code = issue(&test_key(), "Ada", EMAIL);
        let id = code_id(&code).expect("id");
        assert_eq!(id.len(), CODE_ID_CHARS);
        assert!(code.split_once('.').unwrap().1.starts_with(&id));
        assert_eq!(code_id(&format!(" {code} ")), Some(id));
    }

    #[test]
    fn email_shape_check() {
        assert!(looks_like_email(" ada@example.com "));
        assert!(!looks_like_email("ada@example"));
        assert!(!looks_like_email("@example.com"));
        assert!(!looks_like_email("ada lovelace@example.com"));
    }

    #[test]
    fn store_builds_ignore_supporter_codes() {
        let state = SupportState {
            code: Some(issue(&test_key(), "Ada", EMAIL)),
            email_hash: Some(email_hash(EMAIL)),
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
