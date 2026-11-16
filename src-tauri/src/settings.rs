use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use tauri_plugin_global_shortcut::Shortcut;
use ts_rs::TS;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub enum ShortcutMode {
    #[default]
    Hold,
    Toggle,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default)]
    pub version: u32,
    #[serde(default = "default_shortcut")]
    pub shortcut: String,
    #[serde(default)]
    pub shortcut_mode: ShortcutMode,
    #[serde(default)]
    pub microphone: Option<String>,
    #[serde(default)]
    pub launch_at_login: bool,
}

const CURRENT_VERSION: u32 = 1;

fn default_shortcut() -> String {
    "Alt+Shift+Space".into()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            shortcut: default_shortcut(),
            shortcut_mode: ShortcutMode::Hold,
            microphone: None,
            launch_at_login: false,
        }
    }
}

impl Settings {
    pub fn chord(&self) -> Result<Shortcut, String> {
        let chord: Shortcut = self
            .shortcut
            .parse()
            .map_err(|e| format!("Invalid shortcut: {e}"))?;
        if chord.mods.is_empty() {
            return Err("Choose a shortcut with Command, Option, Control or Shift".into());
        }
        Ok(chord)
    }
}

pub fn load(path: &Path) -> Result<Settings, String> {
    match fs::read(path) {
        Ok(bytes) => {
            let mut settings: Settings =
                serde_json::from_slice(&bytes).map_err(|e| format!("Read settings: {e}"))?;
            migrate(&mut settings);
            settings.chord()?;
            Ok(settings)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(error) => Err(format!("Read settings: {error}")),
    }
}

fn migrate(settings: &mut Settings) {
    if settings.version == 0 {
        settings.version = CURRENT_VERSION;
    }
}

pub fn load_or_default(path: &Path) -> (Settings, Option<String>) {
    match load(path) {
        Ok(settings) => (settings, None),
        Err(error) => {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let backup = path.with_file_name(format!("settings.json.bad-{timestamp}"));
            let detail = match fs::rename(path, &backup) {
                Ok(()) => format!("Original saved to {}", backup.display()),
                Err(error) => format!("Could not back up original: {error}"),
            };
            (
                Settings::default(),
                Some(format!("{error}. Using default settings. {detail}")),
            )
        }
    }
}

pub fn save(path: &Path, settings: &Settings) -> Result<(), String> {
    let parent = path.parent().ok_or("Settings directory unavailable")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(&mut file, settings).map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    file.persist(path)
        .map_err(|e| format!("Save settings: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typescript_contract_matches_rust_settings() {
        let contract = format!(
            "export {}\nexport {}\n",
            ShortcutMode::decl(),
            Settings::decl()
        );
        let checked_in = include_str!("../../src/settings.generated.ts");
        let compact = |text: &str| {
            text.chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
        };
        assert_eq!(compact(checked_in), compact(&contract).replace(',', ";"));
    }

    #[test]
    fn settings_survive_restart_and_corruption_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        assert_eq!(load(&path).unwrap(), Settings::default());
        let settings = Settings {
            shortcut: "Control+Shift+KeyD".into(),
            shortcut_mode: ShortcutMode::Toggle,
            microphone: Some("External microphone".into()),
            launch_at_login: true,
            ..Settings::default()
        };
        save(&path, &settings).unwrap();
        assert_eq!(load(&path).unwrap(), settings);
        fs::write(path, "{broken").unwrap();
        assert!(load(&dir.path().join("settings.json")).is_err());
    }

    #[test]
    fn old_and_newer_files_keep_compatible_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"shortcut":"Control+KeyD"}"#).unwrap();
        let settings = load(&path).unwrap();
        assert_eq!(settings.shortcut, "Control+KeyD");
        assert_eq!(settings.version, 1);
        assert_eq!(settings.shortcut_mode, ShortcutMode::Hold);
        assert_eq!(settings.microphone, None);
        assert!(!settings.launch_at_login);
        fs::write(&path, r#"{"version":2,"newSetting":true}"#).unwrap();
        let settings = load(&path).unwrap();
        assert_eq!(settings.version, 2);
        assert_eq!(settings.shortcut, "Alt+Shift+Space");
    }

    #[test]
    fn corrupt_or_invalid_settings_are_backed_up_before_using_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        for original in ["{broken", r#"{"shortcut":"not-a-key"}"#] {
            fs::write(&path, original).unwrap();
            let (settings, error) = load_or_default(&path);
            assert_eq!(settings, Settings::default());
            assert!(error.unwrap().contains("Using default settings"));
            assert!(!path.exists());
            assert!(fs::read_dir(dir.path()).unwrap().any(|entry| {
                let entry = entry.unwrap();
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("settings.json.bad-")
                    && fs::read_to_string(entry.path()).unwrap() == original
            }));
        }
    }

    #[test]
    fn bare_keys_and_invalid_shortcuts_cannot_replace_the_chord() {
        for shortcut in ["Space", "not-a-key"] {
            assert!(
                Settings {
                    shortcut: shortcut.into(),
                    ..Settings::default()
                }
                .chord()
                .is_err()
            );
        }
        assert!(Settings::default().chord().is_ok());
    }
}
