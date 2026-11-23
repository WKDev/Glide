//! Issue and track Glide supporter codes.
//!
//! Keep the key file outside the repository and back it up; codes cannot be
//! issued without it, and anyone holding it can issue them. Every issued
//! code is recorded in `supporters.csv` next to the key file.
//!
//!   cargo run --example supporter_code -- keygen <key-file>
//!   cargo run --example supporter_code -- issue <key-file> "<name>" <email> [platform]
//!   cargo run --example supporter_code -- find <key-file> <text>
//!
//! To revoke a code found shared online, look up its ID with `find` and add
//! it to `REVOKED_CODE_IDS` in src/support.rs.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, process};

use ed25519_dalek::{Signer, SigningKey};
use glide_lib::support;

const LEDGER_HEADER: &str = "issued_at,code_id,name,email,platform";

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

fn ledger_path(key_path: &str) -> PathBuf {
    Path::new(key_path)
        .parent()
        .unwrap_or(Path::new("."))
        .join("supporters.csv")
}

/// Quote a CSV field when it holds a comma, quote, or line break.
fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

/// Split one CSV line written by `csv_field`.
fn csv_fields(line: &str) -> Vec<String> {
    let mut fields = vec![String::new()];
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                chars.next();
                fields.last_mut().unwrap().push('"');
            }
            ('"', _) => quoted = !quoted,
            (',', false) => fields.push(String::new()),
            _ => fields.last_mut().unwrap().push(c),
        }
    }
    fields
}

fn ledger_rows(path: &Path) -> Vec<Vec<String>> {
    fs::read_to_string(path)
        .map(|text| text.lines().skip(1).map(csv_fields).collect())
        .unwrap_or_default()
}

/// UTC date as YYYY-MM-DD (civil-from-days, Howard Hinnant).
fn today_utc() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

fn keygen(path: &str) {
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

fn issue(key_path: &str, name: &str, email: &str, platform: &str) {
    let name = name.trim();
    let email = email.trim().to_lowercase();
    if name.is_empty() || name.chars().count() > support::MAX_NAME_CHARS {
        fail(&format!(
            "name must be 1 to {} characters",
            support::MAX_NAME_CHARS
        ));
    }
    if !support::looks_like_email(&email) {
        fail("email does not look like an address");
    }
    let ledger = ledger_path(key_path);
    if let Some(row) = ledger_rows(&ledger)
        .into_iter()
        .find(|row| row.get(3).map(String::as_str) == Some(email.as_str()))
    {
        fail(&format!(
            "{email} already has a code (issued {}, ID {}). Codes are deterministic, \
             so issuing again would give the same code for the same name.",
            row.first().map_or("?", String::as_str),
            row.get(1).map_or("?", String::as_str)
        ));
    }

    let key = read_key(key_path);
    let hash = support::email_hash(&email);
    let signature = key.sign(&support::signed_message(name, &hash)).to_bytes();
    let code = support::encode_code(name, &signature);
    if support::verify_code(&code, &hash).is_none() {
        fail("this key does not match PUBLIC_KEY in src/support.rs");
    }
    let id = support::code_id(&code).expect("issued code has an ID");

    let new_file = !ledger.exists();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&ledger)
        .unwrap_or_else(|e| fail(&format!("open {}: {e}", ledger.display())));
    if new_file {
        writeln!(file, "{LEDGER_HEADER}").unwrap_or_else(|e| fail(&e.to_string()));
    }
    let row = [today_utc().as_str(), &id, name, &email, platform]
        .map(csv_field)
        .join(",");
    writeln!(file, "{row}").unwrap_or_else(|e| fail(&e.to_string()));

    println!("{code}");
    eprintln!(
        "Recorded {id} in {}. Send the code with this email address: {email}",
        ledger.display()
    );
}

fn find(key_path: &str, needle: &str) {
    let needle = needle.to_lowercase();
    let ledger = ledger_path(key_path);
    let rows: Vec<_> = ledger_rows(&ledger)
        .into_iter()
        .filter(|row| row.iter().any(|f| f.to_lowercase().contains(&needle)))
        .collect();
    if rows.is_empty() {
        fail(&format!("no match in {}", ledger.display()));
    }
    println!("{LEDGER_HEADER}");
    for row in rows {
        println!(
            "{}",
            row.iter()
                .map(|f| csv_field(f))
                .collect::<Vec<_>>()
                .join(",")
        );
    }
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["keygen", path] => keygen(path),
        ["issue", path, name, email] => issue(path, name, email, ""),
        ["issue", path, name, email, platform] => issue(path, name, email, platform),
        ["find", path, needle] => find(path, needle),
        _ => fail(
            "usage:\n  supporter_code keygen <key-file>\n  \
             supporter_code issue <key-file> \"<name>\" <email> [platform]\n  \
             supporter_code find <key-file> <text>",
        ),
    }
}
