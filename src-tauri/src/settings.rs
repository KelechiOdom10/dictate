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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub shortcut: String,
    pub shortcut_mode: ShortcutMode,
    pub microphone: Option<String>,
    pub launch_at_login: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            shortcut: "Alt+Shift+Space".into(),
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
            let settings: Settings =
                serde_json::from_slice(&bytes).map_err(|e| format!("Read settings: {e}"))?;
            settings.chord()?;
            Ok(settings)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(error) => Err(format!("Read settings: {error}")),
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
        };
        save(&path, &settings).unwrap();
        assert_eq!(load(&path).unwrap(), settings);
        fs::write(path, "{broken").unwrap();
        assert!(load(&dir.path().join("settings.json")).is_err());
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
