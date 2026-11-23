//! Issue Glide supporter codes.
//!
//! Keep the key file outside the repository and back it up; codes cannot be
//! issued without it, and anyone holding it can issue them.
//!
//!   cargo run --example supporter_code -- keygen <key-file>
//!   cargo run --example supporter_code -- issue <key-file> "<supporter name>"

use std::{env, fs, process};

use ed25519_dalek::{Signer, SigningKey};
use glide_lib::support;

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    process::exit(1);
}

fn read_key(path: &str) -> SigningKey {
    let text = fs::read_to_string(path).unwrap_or_else(|e| fail(&format!("read {path}: {e}")));
    let bytes: Vec<u8> = (0..64)
        .step_by(2)
        .map(|i| {
            text.trim()
                .get(i..i + 2)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        })
        .collect::<Option<_>>()
        .unwrap_or_else(|| fail("key file must hold 64 hex characters"));
    SigningKey::from_bytes(&bytes.try_into().expect("32 bytes"))
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["keygen", path] => {
            if fs::metadata(path).is_ok() {
                fail(&format!("{path} already exists; refusing to overwrite it"));
            }
            let mut secret = [0u8; 32];
            getrandom::getrandom(&mut secret).unwrap_or_else(|e| fail(&e.to_string()));
            let hex: String = secret.iter().map(|b| format!("{b:02x}")).collect();
            fs::write(path, hex).unwrap_or_else(|e| fail(&format!("write {path}: {e}")));
            let public = SigningKey::from_bytes(&secret).verifying_key().to_bytes();
            println!("Wrote {path}. Put this in src/support.rs as PUBLIC_KEY:");
            println!("{public:?}");
        }
        ["issue", path, name] => {
            let name = name.trim();
            if name.is_empty() || name.chars().count() > support::MAX_NAME_CHARS {
                fail(&format!(
                    "name must be 1 to {} characters",
                    support::MAX_NAME_CHARS
                ));
            }
            let key = read_key(path);
            let signature = key.sign(&support::signed_message(name)).to_bytes();
            let code = support::encode_code(name, &signature);
            if support::verify_code(&code).is_none() {
                fail("this key does not match PUBLIC_KEY in src/support.rs");
            }
            println!("{code}");
        }
        _ => fail("usage: supporter_code keygen <key-file> | issue <key-file> \"<name>\""),
    }
}
