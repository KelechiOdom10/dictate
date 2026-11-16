//! Speech models: the curated whisper.cpp list, and downloading, verifying and
//! deleting their weights in the app data folder.
//!
//! A download streams into `<file>.part`, hashing as it goes. Only a file with
//! the expected length and SHA-256 is renamed into place, so a model file that
//! exists is always a complete one.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq, Hash, TS)]
pub enum SpeechModel {
    #[serde(rename = "tiny.en")]
    TinyEn,
    #[default]
    #[serde(rename = "base.en")]
    BaseEn,
    #[serde(rename = "small.en")]
    SmallEn,
    #[serde(rename = "medium.en")]
    MediumEn,
    #[serde(rename = "large-v3-turbo")]
    LargeV3Turbo,
}

pub struct ModelInfo {
    pub id: SpeechModel,
    pub name: &'static str,
    pub description: &'static str,
    pub file_name: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
    /// Relative accuracy, 1 to 5. An estimate from model size, not measured here.
    pub accuracy: u8,
    /// Median `transcribe` time for the 5.9 s generated-speech fixture on the
    /// owner's M4 Pro. See `docs/spike-results.md`.
    pub measured_ms: u32,
    pub recommended: bool,
}

const HUGGING_FACE: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/";

/// Sizes and SHA-256 digests are the Git LFS object ids published for
/// `ggerganov/whisper.cpp` on Hugging Face, checked against downloads.
pub const CATALOG: [ModelInfo; 5] = [
    ModelInfo {
        id: SpeechModel::TinyEn,
        name: "Tiny English",
        description: "Instant, for short notes. Misses more words.",
        file_name: "ggml-tiny.en.bin",
        bytes: 77_704_715,
        sha256: "921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f",
        accuracy: 1,
        measured_ms: 188,
        recommended: false,
    },
    ModelInfo {
        id: SpeechModel::BaseEn,
        name: "Base English",
        description: "Fast and dependable for everyday dictation.",
        file_name: "ggml-base.en.bin",
        bytes: 147_964_211,
        sha256: "a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002",
        accuracy: 2,
        measured_ms: 232,
        recommended: false,
    },
    ModelInfo {
        id: SpeechModel::SmallEn,
        name: "Small English",
        description: "Noticeably more accurate, still quick on this Mac.",
        file_name: "ggml-small.en.bin",
        bytes: 487_614_201,
        sha256: "c6138d6d58ecc8322097e0f987c32f1be8bb0a18532a3f88f734d1bbf9c41e5d",
        accuracy: 3,
        measured_ms: 437,
        recommended: true,
    },
    ModelInfo {
        id: SpeechModel::MediumEn,
        name: "Medium English",
        description: "Careful with names and jargon, but slower.",
        file_name: "ggml-medium.en.bin",
        bytes: 1_533_774_781,
        sha256: "cc37e93478338ec7700281a7ac30a10128929eb8f427dda2e865faa8f6da4356",
        accuracy: 4,
        measured_ms: 1_037,
        recommended: false,
    },
    ModelInfo {
        id: SpeechModel::LargeV3Turbo,
        name: "Large v3 Turbo",
        description: "The most accurate. Multilingual, set to English here.",
        file_name: "ggml-large-v3-turbo.bin",
        bytes: 1_624_555_275,
        sha256: "1fc70f774d38eb169993ac391eea357ef47c88757ef72ee5943879b7e8e2bc69",
        accuracy: 5,
        measured_ms: 1_084,
        recommended: false,
    },
];

pub fn info(model: SpeechModel) -> &'static ModelInfo {
    CATALOG
        .iter()
        .find(|info| info.id == model)
        .expect("every SpeechModel has a catalog entry")
}

pub fn path(dir: &Path, model: SpeechModel) -> PathBuf {
    dir.join(info(model).file_name)
}

fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    dest.with_file_name(name)
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ModelState {
    Available,
    Downloading {
        #[ts(type = "number")]
        received: u64,
        #[ts(type = "number")]
        total: u64,
        cancelling: bool,
    },
    Installed,
    Failed {
        message: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ModelEntry {
    pub id: SpeechModel,
    pub name: String,
    pub description: String,
    #[ts(type = "number")]
    pub bytes: u64,
    pub accuracy: u8,
    pub measured_ms: u32,
    pub recommended: bool,
    pub active: bool,
    pub state: ModelState,
}

#[derive(Debug, PartialEq)]
pub enum DownloadError {
    Cancelled,
    Network(String),
    Corrupt(String),
    Disk(String),
}

impl DownloadError {
    fn message(&self) -> &'static str {
        match self {
            Self::Cancelled => "Download cancelled.",
            Self::Network(_) => "Download failed. Check your connection and try again.",
            Self::Corrupt(_) => "The download didn’t match the published file. Try again.",
            Self::Disk(_) => "Couldn’t save the model. Check free disk space and try again.",
        }
    }
}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => write!(f, "cancelled"),
            Self::Network(cause) | Self::Corrupt(cause) | Self::Disk(cause) => {
                write!(f, "{} ({cause})", self.message())
            }
        }
    }
}

enum Job {
    Running {
        received: u64,
        total: u64,
        cancel: Arc<AtomicBool>,
    },
    Failed(String),
}

/// Which models are downloading or failed. Installed and available are read
/// from disk, so they never go stale.
#[derive(Default)]
pub struct Downloads {
    jobs: HashMap<SpeechModel, Job>,
}

impl Downloads {
    pub fn begin(&mut self, model: SpeechModel) -> Result<Arc<AtomicBool>, String> {
        if self.is_running(model) {
            return Err(format!("{} is already downloading", info(model).name));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.jobs.insert(
            model,
            Job::Running {
                received: 0,
                total: info(model).bytes,
                cancel: cancel.clone(),
            },
        );
        Ok(cancel)
    }

    pub fn progress(&mut self, model: SpeechModel, bytes: u64) {
        if let Some(Job::Running { received, .. }) = self.jobs.get_mut(&model) {
            *received = bytes;
        }
    }

    /// Ask a running download to stop. It stays `Downloading` (cancelling)
    /// until its worker calls `finish`, so a new download of the same model
    /// cannot race the old one's partial file.
    pub fn cancel(&mut self, model: SpeechModel) -> bool {
        match self.jobs.get(&model) {
            Some(Job::Running { cancel, .. }) => {
                cancel.store(true, Ordering::Relaxed);
                true
            }
            _ => false,
        }
    }

    pub fn finish(&mut self, model: SpeechModel, result: &Result<(), DownloadError>) {
        match result {
            Ok(()) | Err(DownloadError::Cancelled) => {
                self.jobs.remove(&model);
            }
            Err(error) => {
                self.jobs
                    .insert(model, Job::Failed(error.message().to_string()));
            }
        }
    }

    pub fn is_running(&self, model: SpeechModel) -> bool {
        matches!(self.jobs.get(&model), Some(Job::Running { .. }))
    }

    pub fn state(&self, model: SpeechModel, installed: bool) -> ModelState {
        match self.jobs.get(&model) {
            Some(Job::Running {
                received,
                total,
                cancel,
            }) => ModelState::Downloading {
                received: *received,
                total: *total,
                cancelling: cancel.load(Ordering::Relaxed),
            },
            Some(Job::Failed(message)) if !installed => ModelState::Failed {
                message: message.clone(),
            },
            _ if installed => ModelState::Installed,
            _ => ModelState::Available,
        }
    }

    pub fn entries(&self, dir: &Path, active: SpeechModel) -> Vec<ModelEntry> {
        CATALOG
            .iter()
            .map(|info| ModelEntry {
                id: info.id,
                name: info.name.into(),
                description: info.description.into(),
                bytes: info.bytes,
                accuracy: info.accuracy,
                measured_ms: info.measured_ms,
                recommended: info.recommended,
                active: info.id == active,
                state: self.state(info.id, path(dir, info.id).is_file()),
            })
            .collect()
    }
}

/// Stream `source` into `dest`, keeping it only if it is exactly
/// `expected_len` bytes with the expected SHA-256. Any failure removes the
/// partial file and leaves `dest` untouched.
pub fn write_verified(
    mut source: impl Read,
    expected_len: u64,
    expected_sha256: &str,
    dest: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64),
) -> Result<(), DownloadError> {
    let part = part_path(dest);
    let result = (|| {
        let disk = |e: std::io::Error| DownloadError::Disk(format!("{}: {e}", part.display()));
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(disk)?;
        }
        let mut file = fs::File::create(&part).map_err(disk)?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0; 256 * 1024];
        let mut received = 0u64;
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(DownloadError::Cancelled);
            }
            let read = match source.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => read,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(DownloadError::Network(e.to_string())),
            };
            received += read as u64;
            if received > expected_len {
                return Err(DownloadError::Corrupt(format!(
                    "more than the expected {expected_len} bytes"
                )));
            }
            hasher.update(&buffer[..read]);
            file.write_all(&buffer[..read]).map_err(disk)?;
            progress(received);
        }
        if received != expected_len {
            return Err(DownloadError::Network(format!(
                "stopped at {received} of {expected_len} bytes"
            )));
        }
        let digest = hex(&hasher.finalize());
        if digest != expected_sha256 {
            return Err(DownloadError::Corrupt(format!(
                "SHA-256 {digest}, expected {expected_sha256}"
            )));
        }
        file.sync_all().map_err(disk)?;
        drop(file);
        fs::rename(&part, dest).map_err(disk)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&part);
    }
    result
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Download one catalog model into `dir` over HTTPS.
pub fn fetch(
    model: SpeechModel,
    dir: &Path,
    cancel: &AtomicBool,
    progress: impl FnMut(u64),
) -> Result<(), DownloadError> {
    let info = info(model);
    let url = format!("{HUGGING_FACE}{}", info.file_name);
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(std::time::Duration::from_secs(15)))
        .build()
        .into();
    let response = agent
        .get(&url)
        .call()
        .map_err(|e| DownloadError::Network(format!("{url}: {e}")))?;
    write_verified(
        response.into_body().into_reader(),
        info.bytes,
        info.sha256,
        &path(dir, model),
        cancel,
        progress,
    )
}

pub fn delete(dir: &Path, model: SpeechModel) -> Result<(), String> {
    match fs::remove_file(path(dir, model)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("Couldn’t delete {}: {e}", info(model).name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HELLO_SHA256: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

    fn write(
        source: &[u8],
        len: u64,
        sha: &str,
        cancel: bool,
    ) -> (Result<(), DownloadError>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let result = write_verified(
            source,
            len,
            sha,
            &dir.path().join("model.bin"),
            &AtomicBool::new(cancel),
            |_| {},
        );
        (result, dir)
    }

    fn files(dir: &Path) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn verified_download_lands_in_place_and_reports_progress() {
        let dir = tempfile::tempdir().unwrap();
        let mut seen = Vec::new();
        write_verified(
            &b"hello"[..],
            5,
            HELLO_SHA256,
            &dir.path().join("models/model.bin"),
            &AtomicBool::new(false),
            |received| seen.push(received),
        )
        .unwrap();
        assert_eq!(
            fs::read(dir.path().join("models/model.bin")).unwrap(),
            b"hello"
        );
        assert_eq!(files(&dir.path().join("models")), ["model.bin"]);
        assert_eq!(seen.last(), Some(&5));
    }

    #[test]
    fn checksum_mismatch_keeps_nothing() {
        let (result, dir) = write(b"hellp", 5, HELLO_SHA256, false);
        assert!(
            matches!(result, Err(DownloadError::Corrupt(cause)) if cause.contains("expected 2cf24d"))
        );
        assert!(files(dir.path()).is_empty());
    }

    #[test]
    fn short_or_long_downloads_keep_nothing() {
        let (result, dir) = write(b"hell", 5, HELLO_SHA256, false);
        assert!(matches!(result, Err(DownloadError::Network(cause)) if cause.contains("4 of 5")));
        assert!(files(dir.path()).is_empty());
        let (result, dir) = write(b"hello!", 5, HELLO_SHA256, false);
        assert!(matches!(result, Err(DownloadError::Corrupt(_))));
        assert!(files(dir.path()).is_empty());
    }

    #[test]
    fn cancelled_download_keeps_nothing() {
        let (result, dir) = write(b"hello", 5, HELLO_SHA256, true);
        assert_eq!(result, Err(DownloadError::Cancelled));
        assert!(files(dir.path()).is_empty());
    }

    #[test]
    fn failed_download_leaves_an_installed_model_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("model.bin");
        fs::write(&dest, b"hello").unwrap();
        let result = write_verified(
            &b"junk!"[..],
            5,
            HELLO_SHA256,
            &dest,
            &AtomicBool::new(false),
            |_| {},
        );
        assert!(result.is_err());
        assert_eq!(fs::read(&dest).unwrap(), b"hello");
        assert_eq!(files(dir.path()), ["model.bin"]);
    }

    #[test]
    fn download_state_machine() {
        let model = SpeechModel::SmallEn;
        let total = info(model).bytes;
        let mut downloads = Downloads::default();
        assert_eq!(downloads.state(model, false), ModelState::Available);
        assert_eq!(downloads.state(model, true), ModelState::Installed);

        let cancel = downloads.begin(model).unwrap();
        assert!(downloads.begin(model).is_err(), "one download per model");
        downloads.progress(model, 42);
        assert_eq!(
            downloads.state(model, false),
            ModelState::Downloading {
                received: 42,
                total,
                cancelling: false
            }
        );

        assert!(downloads.cancel(model));
        assert!(cancel.load(Ordering::Relaxed));
        assert!(
            downloads.begin(model).is_err(),
            "cancelling still owns the file"
        );
        assert!(matches!(
            downloads.state(model, false),
            ModelState::Downloading {
                cancelling: true,
                ..
            }
        ));
        downloads.finish(model, &Err(DownloadError::Cancelled));
        assert_eq!(downloads.state(model, false), ModelState::Available);
        assert!(!downloads.cancel(model));

        downloads.begin(model).unwrap();
        downloads.finish(model, &Err(DownloadError::Corrupt("sha".into())));
        assert_eq!(
            downloads.state(model, false),
            ModelState::Failed {
                message: "The download didn’t match the published file. Try again.".into()
            }
        );
        downloads.progress(model, 7);
        assert!(matches!(
            downloads.state(model, false),
            ModelState::Failed { .. }
        ));

        downloads.begin(model).unwrap();
        downloads.finish(model, &Ok(()));
        assert_eq!(downloads.state(model, true), ModelState::Installed);
        assert_eq!(
            downloads.state(SpeechModel::TinyEn, false),
            ModelState::Available
        );
    }

    #[test]
    fn entries_read_installed_models_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(path(dir.path(), SpeechModel::TinyEn), b"x").unwrap();
        fs::write(dir.path().join("ggml-base.en.bin.part"), b"x").unwrap();
        let entries = Downloads::default().entries(dir.path(), SpeechModel::TinyEn);
        assert_eq!(entries.len(), 5);
        assert_eq!(entries[0].state, ModelState::Installed);
        assert!(entries[0].active);
        assert_eq!(entries[1].state, ModelState::Available);
        assert_eq!(entries.iter().filter(|entry| entry.recommended).count(), 1);
        delete(dir.path(), SpeechModel::TinyEn).unwrap();
        delete(dir.path(), SpeechModel::TinyEn).unwrap();
        assert!(!path(dir.path(), SpeechModel::TinyEn).exists());
    }

    /// The Models stories render `src/models.fixture.json`; this keeps the
    /// sample catalog in step with the real one.
    #[test]
    fn story_fixture_matches_the_catalog() {
        let dir = tempfile::tempdir().unwrap();
        let real =
            serde_json::to_value(Downloads::default().entries(dir.path(), SpeechModel::BaseEn))
                .unwrap();
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../src/models.fixture.json")).unwrap();
        assert_eq!(fixture, real);
    }

    #[test]
    fn typescript_contract_matches_rust_models() {
        let contract = format!(
            "export {}\nexport {}\nexport {}\n",
            SpeechModel::decl(),
            ModelState::decl(),
            ModelEntry::decl()
        );
        let checked_in = include_str!("../../src/models.generated.ts");
        // Prettier unquotes `kind`, leads multi-line unions with `|` and drops
        // a single-line object's last `;`.
        let compact = |text: &str| {
            text.chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
                .replace(',', ";")
                .replace("\"kind\"", "kind")
                .replace("=|", "=")
                .replace(";}", "}")
        };
        assert_eq!(compact(checked_in), compact(&contract));
    }

    /// Downloads tiny.en (78 MB) from Hugging Face through `fetch`.
    #[test]
    #[ignore = "Downloads 78 MB; run explicitly with --ignored --nocapture"]
    fn fetch_downloads_and_verifies_a_real_model() {
        let dir = tempfile::tempdir().unwrap();
        let started = std::time::Instant::now();
        let mut updates = 0;
        fetch(
            SpeechModel::TinyEn,
            dir.path(),
            &AtomicBool::new(false),
            |_| updates += 1,
        )
        .unwrap();
        let bytes = fs::metadata(path(dir.path(), SpeechModel::TinyEn))
            .unwrap()
            .len();
        println!(
            "fetched tiny.en bytes={bytes} progress_updates={updates} ms={}",
            started.elapsed().as_millis()
        );
        assert_eq!(bytes, info(SpeechModel::TinyEn).bytes);
        assert_eq!(files(dir.path()), ["ggml-tiny.en.bin"]);
    }
}
