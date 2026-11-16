use crate::models::SpeechModel;
use crate::problem::Problem;
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

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    #[default]
    Ember,
    Paper,
    Capy,
    RosePine,
    Neutral,
}

/// A theme id this build doesn't know, such as one written by a newer build
/// or by hand, falls back to Ember instead of discarding every setting.
fn theme_or_ember<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Theme, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).unwrap_or_default())
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub enum AppearanceMode {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub enum OverlayPosition {
    Top,
    #[default]
    Bottom,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub enum PasteMethod {
    #[default]
    Paste,
    CopyOnly,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub enum MicMode {
    #[default]
    OnDemand,
    Warm,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub enum HistoryRetention {
    Day,
    #[default]
    Week,
    Month,
    Forever,
}

impl HistoryRetention {
    pub fn days(self) -> Option<i64> {
        match self {
            Self::Day => Some(1),
            Self::Week => Some(7),
            Self::Month => Some(30),
            Self::Forever => None,
        }
    }
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
    #[serde(default, deserialize_with = "theme_or_ember")]
    pub theme: Theme,
    #[serde(default)]
    pub appearance_mode: AppearanceMode,
    #[serde(default)]
    pub overlay_position: OverlayPosition,
    #[serde(default)]
    pub history_retention: HistoryRetention,
    #[serde(default)]
    pub speech_model: SpeechModel,
    #[serde(default)]
    pub paste_method: PasteMethod,
    #[serde(default = "enabled")]
    pub restore_clipboard: bool,
    #[serde(default)]
    pub trailing_space: bool,
    #[serde(default)]
    pub capitalize_first: bool,
    #[serde(default)]
    pub mic_mode: MicMode,
    #[serde(default)]
    pub sound_feedback: bool,
    #[serde(default)]
    pub debug_recordings: bool,
    #[serde(default = "default_annotate_shortcut")]
    pub annotate_shortcut: String,
    /// Off: the Annotate shortcut is not registered at all.
    #[serde(default = "enabled")]
    pub annotate_enabled: bool,
    /// On: the recording also shows dictate's ink and floating bar.
    #[serde(default)]
    pub annotate_show_in_recordings: bool,
}

fn enabled() -> bool {
    true
}

const CURRENT_VERSION: u32 = 1;

fn default_shortcut() -> String {
    "Alt+Shift+Space".into()
}

fn default_annotate_shortcut() -> String {
    "Alt+Shift+KeyA".into()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            shortcut: default_shortcut(),
            shortcut_mode: ShortcutMode::Hold,
            microphone: None,
            launch_at_login: false,
            theme: Theme::Ember,
            appearance_mode: AppearanceMode::System,
            overlay_position: OverlayPosition::Bottom,
            history_retention: HistoryRetention::Week,
            speech_model: SpeechModel::ParakeetUnifiedEn,
            paste_method: PasteMethod::Paste,
            restore_clipboard: true,
            trailing_space: false,
            capitalize_first: false,
            mic_mode: MicMode::OnDemand,
            sound_feedback: false,
            debug_recordings: false,
            annotate_shortcut: default_annotate_shortcut(),
            annotate_enabled: true,
            annotate_show_in_recordings: false,
        }
    }
}

pub struct SettingsChanges {
    pub shortcut: Option<(Shortcut, Shortcut)>,
    /// The Annotate chord that should be registered after the save: `None`
    /// when Annotate is off. Only set when it differs from before.
    pub annotate: Option<Option<Shortcut>>,
}

impl Settings {
    pub fn validate_changes(
        &self,
        previous: &Settings,
        input_devices: impl FnOnce() -> Result<Vec<String>, String>,
        model_installed: impl FnOnce(SpeechModel) -> bool,
    ) -> Result<SettingsChanges, String> {
        if self.speech_model != previous.speech_model && !model_installed(self.speech_model) {
            return Err(format!(
                "Download {} before choosing it",
                crate::models::info(self.speech_model).name
            ));
        }
        let annotate = if (self.annotate_enabled, &self.annotate_shortcut)
            != (previous.annotate_enabled, &previous.annotate_shortcut)
        {
            let chord = self.annotate_chord()?;
            if self.chord().ok() == Some(chord) {
                return Err("That shortcut is your dictation shortcut. Choose another.".into());
            }
            let old = previous
                .annotate_enabled
                .then(|| previous.annotate_chord().ok())
                .flatten();
            let new = self.annotate_enabled.then_some(chord);
            (old != new).then_some(new)
        } else {
            None
        };
        let shortcut = if self.shortcut != previous.shortcut {
            let old = previous.chord()?;
            let new = self.chord()?;
            if self.annotate_chord().ok() == Some(new) {
                return Err("That shortcut starts Annotate. Choose another.".into());
            }
            (old != new).then_some((old, new))
        } else {
            None
        };
        if self.microphone != previous.microphone
            && let Some(name) = &self.microphone
            && !input_devices()?.contains(name)
        {
            return Err(format!("Microphone unavailable: {name}"));
        }
        Ok(SettingsChanges { shortcut, annotate })
    }

    pub fn chord(&self) -> Result<Shortcut, String> {
        parse_chord(&self.shortcut)
    }

    /// Tap to start an Annotate session, tap again to stop.
    pub fn annotate_chord(&self) -> Result<Shortcut, String> {
        parse_chord(&self.annotate_shortcut)
    }
}

fn parse_chord(shortcut: &str) -> Result<Shortcut, String> {
    let chord: Shortcut = shortcut
        .parse()
        .map_err(|e| format!("Invalid shortcut: {e}"))?;
    if chord.mods.is_empty() {
        return Err("Choose a shortcut with Command, Option, Control or Shift".into());
    }
    Ok(chord)
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

pub fn load_or_default(path: &Path) -> (Settings, Option<Problem>) {
    match load(path) {
        Ok(settings) => (settings, None),
        Err(error) => {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let backup = path.with_file_name(format!("settings.json.bad-{timestamp}"));
            let (fix, detail) = match fs::rename(path, &backup) {
                Ok(()) => (
                    "Using defaults. Your old file is saved beside them.",
                    format!("{error}; original saved to {}", backup.display()),
                ),
                Err(backup_error) => (
                    "Using defaults. Choose your preferences again in Settings.",
                    format!("{error}; could not back up original: {backup_error}"),
                ),
            };
            (
                Settings::default(),
                Some(Problem::new("Settings couldn’t be read", fix).because(detail)),
            )
        }
    }
}

pub fn save(path: &Path, settings: &Settings) -> Result<(), String> {
    let parent = path.parent().ok_or("Settings directory unavailable")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    let mut value = match fs::read(path) {
        Ok(bytes) => serde_json::from_slice::<serde_json::Map<String, serde_json::Value>>(&bytes)
            .unwrap_or_default(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => serde_json::Map::new(),
        Err(error) => return Err(format!("Read existing settings: {error}")),
    };
    let serde_json::Value::Object(known) =
        serde_json::to_value(settings).map_err(|e| e.to_string())?
    else {
        return Err("Settings must be an object".into());
    };
    value.extend(known);
    serde_json::to_writer_pretty(&mut file, &value).map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    file.persist(path)
        .map_err(|e| format!("Save settings: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_replaces_corrupt_settings_left_after_failed_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        for corrupt in ["{broken", "[]", "null"] {
            fs::write(&path, corrupt).unwrap();
            let settings = Settings {
                theme: Theme::Paper,
                ..Settings::default()
            };
            save(&path, &settings).unwrap();
            assert_eq!(load(&path).unwrap(), settings);
        }
    }

    #[test]
    fn typescript_contract_matches_rust_settings() {
        let contract = format!(
            "import type {{ SpeechModel }} from \"./models.generated\";\nexport {}\nexport {}\nexport {}\nexport {}\nexport {}\nexport {}\nexport {}\nexport {}\n",
            ShortcutMode::decl(),
            Theme::decl(),
            AppearanceMode::decl(),
            OverlayPosition::decl(),
            PasteMethod::decl(),
            MicMode::decl(),
            HistoryRetention::decl(),
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
            theme: Theme::Paper,
            appearance_mode: AppearanceMode::Dark,
            overlay_position: OverlayPosition::Top,
            history_retention: HistoryRetention::Forever,
            speech_model: SpeechModel::SmallEn,
            paste_method: PasteMethod::CopyOnly,
            restore_clipboard: false,
            trailing_space: true,
            capitalize_first: true,
            mic_mode: MicMode::Warm,
            sound_feedback: true,
            debug_recordings: true,
            ..Settings::default()
        };
        save(&path, &settings).unwrap();
        assert_eq!(load(&path).unwrap(), settings);
        fs::write(path, "{broken").unwrap();
        assert!(load(&dir.path().join("settings.json")).is_err());
    }

    #[test]
    fn every_theme_id_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        for (theme, id) in [
            (Theme::Ember, "ember"),
            (Theme::Paper, "paper"),
            (Theme::Capy, "capy"),
            (Theme::RosePine, "rosePine"),
            (Theme::Neutral, "neutral"),
        ] {
            let settings = Settings {
                theme,
                ..Settings::default()
            };
            save(&path, &settings).unwrap();
            let value: serde_json::Value =
                serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            assert_eq!(value["theme"], id);
            assert_eq!(load(&path).unwrap(), settings);
        }
    }

    #[test]
    fn unknown_theme_falls_back_to_ember_and_keeps_other_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        for theme in [r#""dracula""#, r#""Ember""#, "42", "null", r#"{"id":"capy"}"#] {
            fs::write(
                &path,
                format!(r#"{{"shortcut":"Control+KeyD","theme":{theme},"appearanceMode":"dark"}}"#),
            )
            .unwrap();
            let (settings, problem) = load_or_default(&path);
            assert!(problem.is_none(), "{theme}");
            assert_eq!(settings.theme, Theme::Ember, "{theme}");
            assert_eq!(settings.shortcut, "Control+KeyD", "{theme}");
            assert_eq!(settings.appearance_mode, AppearanceMode::Dark, "{theme}");
        }
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
        assert_eq!(settings.theme, Theme::Ember);
        assert_eq!(settings.appearance_mode, AppearanceMode::System);
        assert_eq!(settings.overlay_position, OverlayPosition::Bottom);
        assert_eq!(settings.history_retention, HistoryRetention::Week);
        assert_eq!(settings.speech_model, SpeechModel::ParakeetUnifiedEn);
        assert_eq!(settings.paste_method, PasteMethod::Paste);
        assert!(settings.restore_clipboard);
        assert!(!settings.trailing_space);
        assert!(!settings.capitalize_first);
        assert_eq!(settings.mic_mode, MicMode::OnDemand);
        assert!(!settings.sound_feedback);
        assert!(!settings.debug_recordings);
        assert_eq!(settings.annotate_shortcut, "Alt+Shift+KeyA");
        fs::write(&path, r#"{"version":2,"newSetting":true}"#).unwrap();
        let settings = load(&path).unwrap();
        assert_eq!(settings.version, 2);
        assert_eq!(settings.shortcut, "Alt+Shift+Space");
        let updated = Settings {
            launch_at_login: true,
            ..settings
        };
        save(&path, &updated).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(value["newSetting"], true);
        assert_eq!(value["version"], 2);
        assert_eq!(value["launchAtLogin"], true);
    }

    /// "Unload model after N minutes idle" was dropped because whisper-cli
    /// runs once per take, so no model stays loaded. A file that has the key
    /// still loads.
    #[test]
    fn a_dropped_unload_model_setting_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{"version":1,"soundFeedback":true,"unloadModelAfterMinutes":5}"#,
        )
        .unwrap();
        let settings = load(&path).unwrap();
        assert!(settings.sound_feedback);
        assert_eq!(
            serde_json::to_value(&settings)
                .unwrap()
                .get("unloadModelAfterMinutes"),
            None
        );
    }

    #[test]
    fn corrupt_or_invalid_settings_are_backed_up_before_using_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        for original in ["{broken", r#"{"shortcut":"not-a-key"}"#] {
            fs::write(&path, original).unwrap();
            let (settings, error) = load_or_default(&path);
            assert_eq!(settings, Settings::default());
            let problem = error.unwrap();
            assert_eq!(problem.title, "Settings couldn’t be read");
            assert!(problem.fix.starts_with("Using defaults."));
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
        assert!(Settings::default().annotate_chord().is_ok());
    }

    #[test]
    fn turning_annotate_off_and_on_changes_what_is_registered() {
        let on = Settings::default();
        let off = Settings {
            annotate_enabled: false,
            ..on.clone()
        };
        let no_devices = || Ok(Vec::new());
        assert_eq!(
            off.validate_changes(&on, no_devices, |_| true)
                .unwrap()
                .annotate,
            Some(None)
        );
        assert_eq!(
            on.validate_changes(&off, no_devices, |_| true)
                .unwrap()
                .annotate,
            Some(Some(on.annotate_chord().unwrap()))
        );
        let screen_only = Settings {
            annotate_show_in_recordings: true,
            ..on.clone()
        };
        assert_eq!(
            screen_only
                .validate_changes(&on, no_devices, |_| true)
                .unwrap()
                .annotate,
            None
        );
    }

    #[test]
    fn settings_saved_before_annotate_options_existed_turn_it_on_and_hide_our_windows() {
        let settings: Settings = serde_json::from_str("{}").unwrap();
        assert!(settings.annotate_enabled);
        assert!(!settings.annotate_show_in_recordings);
    }

    #[test]
    fn the_annotate_shortcut_can_change_but_never_to_the_dictation_one() {
        let previous = Settings::default();
        let no_devices = || Ok(Vec::new());
        let changed = Settings {
            annotate_shortcut: "Alt+Shift+KeyD".into(),
            ..previous.clone()
        };
        let changes = changed
            .validate_changes(&previous, no_devices, |_| true)
            .unwrap();
        assert_eq!(
            changes.annotate,
            Some(Some(changed.annotate_chord().unwrap()))
        );
        let taken = Settings {
            annotate_shortcut: previous.shortcut.clone(),
            ..previous.clone()
        };
        assert_eq!(
            taken
                .validate_changes(&previous, no_devices, |_| true)
                .err()
                .as_deref(),
            Some("That shortcut is your dictation shortcut. Choose another.")
        );
        let bare = Settings {
            annotate_shortcut: "KeyS".into(),
            ..previous.clone()
        };
        assert!(
            bare.validate_changes(&previous, no_devices, |_| true)
                .is_err()
        );
        let clash = Settings {
            shortcut: previous.annotate_shortcut.clone(),
            ..previous.clone()
        };
        assert_eq!(
            clash
                .validate_changes(&previous, no_devices, |_| true)
                .err()
                .as_deref(),
            Some("That shortcut starts Annotate. Choose another.")
        );
    }
}
