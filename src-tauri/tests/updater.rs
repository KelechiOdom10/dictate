//! The update path that can be proven without a second Mac: an endpoint that
//! serves a well-formed manifest, a signature that verifies against the public
//! key, and signatures and payloads that do not verify being rejected.
//!
//! `fixtures/updater/` holds a public key, a payload and its signature, made
//! once with a throwaway key whose private half was discarded. The updater
//! plugin runs this same minisign check before it installs anything; what
//! needs two machines is the install and relaunch (docs/release.md).

use std::{
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
};

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

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/updater")
            .join(name),
    )
    .unwrap()
}

fn text(name: &str) -> String {
    String::from_utf8(fixture(name)).unwrap().trim().to_string()
}

fn get(url: &str) -> Vec<u8> {
    ureq::get(url)
        .call()
        .unwrap()
        .into_body()
        .read_to_vec()
        .unwrap()
}

/// The manifest `scripts/publish-update.sh` writes, pointing at `base`.
fn manifest(base: &str, signature: &str) -> Vec<u8> {
    serde_json::json!({
        "version": "0.2.0",
        "notes": "fixture",
        "pub_date": "2026-11-16T12:00:00Z",
        "platforms": {
            "darwin-aarch64": { "signature": signature, "url": format!("{base}/updates/payload.txt") }
        }
    })
    .to_string()
    .into_bytes()
}

#[test]
fn the_endpoint_serves_a_manifest_whose_artifact_verifies_against_the_public_key() {
    let base = serve_with_manifest(&text("payload.txt.sig"));
    let manifest: serde_json::Value =
        serde_json::from_slice(&get(&format!("{base}/updates/latest.json"))).unwrap();
    let platform = &manifest["platforms"]["darwin-aarch64"];
    assert_eq!(manifest["version"], "0.2.0");
    assert!(manifest["pub_date"].as_str().unwrap().ends_with('Z'));
    let artifact = get(platform["url"].as_str().unwrap());
    assert_eq!(artifact, fixture("payload.txt"));
    verify(
        &text("test.key.pub"),
        platform["signature"].as_str().unwrap(),
        &artifact,
    )
    .unwrap();
}

/// The manifest's artifact URL has to name the server it is served from, so
/// the listener is bound first and the routes filled in from its address.
fn serve_with_manifest(signature: &str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let routes = [
        ("/updates/latest.json", manifest(&base, signature)),
        ("/updates/payload.txt", fixture("payload.txt")),
    ];
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut request = [0u8; 2048];
            let n = stream.read(&mut request).unwrap_or(0);
            let line = String::from_utf8_lossy(&request[..n]);
            let path = line.split_whitespace().nth(1).unwrap_or("/");
            let (status, body) = match routes.iter().find(|(route, _)| *route == path) {
                Some((_, body)) => ("200 OK", body.clone()),
                None => ("404 Not Found", Vec::new()),
            };
            let _ = write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(&body);
        }
    });
    base
}

#[test]
fn a_corrupted_signature_is_rejected() {
    let engine = base64::engine::general_purpose::STANDARD;
    let signature = engine.decode(text("payload.txt.sig")).unwrap();
    let mut lines: Vec<String> = String::from_utf8(signature)
        .unwrap()
        .lines()
        .map(String::from)
        .collect();
    let mut bytes = engine.decode(&lines[1]).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0x01;
    lines[1] = engine.encode(bytes);
    let corrupted = engine.encode(lines.join("\n") + "\n");
    let error = verify(&text("test.key.pub"), &corrupted, &fixture("payload.txt")).unwrap_err();
    assert!(!error.is_empty());
    assert!(
        verify(
            &text("test.key.pub"),
            "not a signature",
            &fixture("payload.txt")
        )
        .is_err()
    );
}

#[test]
fn a_tampered_payload_is_rejected() {
    let mut payload = fixture("payload.txt");
    payload[0] ^= 0x01;
    assert!(verify(&text("test.key.pub"), &text("payload.txt.sig"), &payload).is_err());
}

#[test]
fn a_signature_made_by_another_key_is_rejected() {
    assert!(
        verify(
            &text("other.key.pub"),
            &text("payload.txt.sig"),
            &fixture("payload.txt")
        )
        .is_err()
    );
}
