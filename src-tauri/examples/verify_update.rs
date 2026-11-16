//! Checks an update artifact against a public key before it is published.
//!
//!     cargo run --example verify_update -- <pubkey> <artifact> <signature-file>
//!
//! `<pubkey>` is the `pubkey` value from `tauri.conf.json`. Exits non-zero
//! unless the signature is valid, so `scripts/publish-update.sh` stops there.

use base64::Engine;
use minisign_verify::{PublicKey, Signature};

/// What tauri-plugin-updater does with the config's `pubkey` and the
/// manifest's `signature`: both are base64 of the minisign text files.
fn verify(pubkey_b64: &str, signature_b64: &str, data: &[u8]) -> Result<(), String> {
    let decode = |text: &str| -> Result<String, String> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(text.trim())
            .map_err(|e| e.to_string())?;
        String::from_utf8(bytes).map_err(|e| e.to_string())
    };
    let key = PublicKey::decode(&decode(pubkey_b64)?).map_err(|e| e.to_string())?;
    let signature = Signature::decode(&decode(signature_b64)?).map_err(|e| e.to_string())?;
    key.verify(data, &signature, true)
        .map_err(|e| e.to_string())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [pubkey, artifact, signature] = args.as_slice() else {
        eprintln!("usage: verify_update <pubkey> <artifact> <signature-file>");
        std::process::exit(2);
    };
    let read = |path: &str| {
        std::fs::read(path).unwrap_or_else(|e| {
            eprintln!("{path}: {e}");
            std::process::exit(2);
        })
    };
    let signature = String::from_utf8_lossy(&read(signature)).into_owned();
    match verify(pubkey, &signature, &read(artifact)) {
        Ok(()) => println!("signature valid: {artifact}"),
        Err(error) => {
            eprintln!("signature INVALID: {error}");
            std::process::exit(1);
        }
    }
}
