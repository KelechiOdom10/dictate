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
    pub theme_light: Theme,
    #[serde(default, deserialize_with = "theme_or_ember")]
    pub theme_dark: Theme,
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

const CURRENT_VERSION: u32 = 2;

/// Before v2 there was one theme id for both appearances. A file that still
/// carries it has it copied into both, so upgrading keeps the look that was
/// chosen, and the dead key is dropped so a later save cannot resurrect it.
fn migrate_theme(value: &mut serde_json::Map<String, serde_json::Value>) {
    let Some(legacy) = value.remove("theme") else {
        return;
    };
    if value.contains_key("themeLight") || value.contains_key("themeDark") {
        return;
    }
    value.insert("themeLight".into(), legacy.clone());
    value.insert("themeDark".into(), legacy);
}

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
            theme_light: Theme::Ember,
            theme_dark: Theme::Ember,
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
            let new = if self.annotate_enabled {
                let chord = self.annotate_chord()?;
                if self.chord().ok() == Some(chord) {
                    return Err("That shortcut is your dictation shortcut. Choose another.".into());
                }
                Some(chord)
            } else {
                None
            };
            let old = previous
                .annotate_enabled
                .then(|| previous.annotate_chord().ok())
                .flatten();
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

    /// The theme that applies in the given appearance. Callers resolve
    /// `AppearanceMode::System` first, since only the window server knows.
    pub fn theme_for(&self, dark: bool) -> Theme {
        if dark {
            self.theme_dark
        } else {
            self.theme_light
        }
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
    check_chord(shortcut).map_err(|error| error.message())
}

enum ChordError {
    Invalid(String),
    NoModifier,
    Reserved,
}

impl ChordError {
    fn message(&self) -> String {
        match self {
            Self::Invalid(error) => format!("Invalid shortcut: {error}"),
            Self::NoModifier => "Choose a shortcut with Command, Option, Control or Shift".into(),
            Self::Reserved => "That shortcut is reserved by macOS. Choose another.".into(),
        }
    }

    /// The same problem as a clause for a sentence about a reset shortcut.
    fn reason(&self) -> &'static str {
        match self {
            Self::Invalid(_) => "dictate couldn’t read it",
            Self::NoModifier => "it had no Command, Option, Control or Shift key",
            Self::Reserved => "macOS reserves it",
        }
    }
}

fn check_chord(shortcut: &str) -> Result<Shortcut, ChordError> {
    let chord = shortcut
        .parse::<Shortcut>()
        .map_err(|e| ChordError::Invalid(e.to_string()))?;
    if chord.mods.is_empty() {
        return Err(ChordError::NoModifier);
    }
    if [
        "Super+KeyQ",
        "Super+KeyW",
        "Super+KeyH",
        "Super+Tab",
        "Super+Space",
        "Super+Alt+Escape",
        "Control+Super+KeyQ",
    ]
    .iter()
    .any(|reserved| reserved.parse::<Shortcut>().ok() == Some(chord))
    {
        return Err(ChordError::Reserved);
    }
    Ok(chord)
}

/// A chord the way the Settings window shows it, for a sentence.
fn spoken_chord(shortcut: &str) -> String {
    shortcut
        .split('+')
        .filter(|key| !key.is_empty())
        .map(|key| match key {
            "Super" | "Meta" | "Command" | "CommandOrControl" => "Command".to_string(),
            "Alt" | "Option" => "Option".to_string(),
            key => key
                .trim_start_matches("Key")
                .trim_start_matches("Digit")
                .to_string(),
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

/// One field of the file that couldn't be used and went back to a default.
struct Repair {
    field: String,
    notice: String,
}

/// What a file that couldn't be read keeps: nothing is pruned until the
/// person picks how long history stays.
const UNREADABLE_RETENTION: HistoryRetention = HistoryRetention::Forever;

/// Reads the file field by field. A field that doesn't parse goes back to its
/// default and every other field is kept, so one bad value never costs the
/// rest. `None` when there is no file; an error only when the file is not a
/// JSON object at all.
fn read(path: &Path) -> Result<Option<(Settings, Vec<Repair>)>, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Read settings: {error}")),
    };
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("Read settings: {e}"))?;
    let serde_json::Value::Object(mut stored) = value else {
        return Err("Read settings: not a JSON object".into());
    };
    migrate_theme(&mut stored);
    let serde_json::Value::Object(mut merged) =
        serde_json::to_value(Settings::default()).map_err(|e| e.to_string())?
    else {
        return Err("Settings must be an object".into());
    };
    let mut repairs = Vec::new();
    for (key, value) in stored {
        let Some(previous) = merged.insert(key.clone(), value) else {
            merged.remove(&key);
            continue;
        };
        if serde_json::from_value::<Settings>(serde_json::Value::Object(merged.clone())).is_err() {
            merged.insert(key.clone(), previous);
            repairs.push(Repair {
                notice: format!(
                    "Your {} setting couldn’t be read, so it’s back to its default.",
                    field_label(&key)
                ),
                field: key,
            });
        }
    }
    let mut settings: Settings =
        serde_json::from_value(serde_json::Value::Object(merged)).map_err(|e| e.to_string())?;
    migrate(&mut settings);
    if repairs
        .iter()
        .any(|repair| repair.field == "historyRetention")
    {
        settings.history_retention = UNREADABLE_RETENTION;
        if let Some(repair) = repairs.iter_mut().find(|r| r.field == "historyRetention") {
            repair.notice = "Your history setting couldn’t be read, so dictate keeps every dictation until you choose again.".into();
        }
    }
    let defaults = Settings::default();
    for (field, name, shortcut, default) in [
        (
            "shortcut",
            "dictation",
            &mut settings.shortcut,
            &defaults.shortcut,
        ),
        (
            "annotateShortcut",
            "Annotate",
            &mut settings.annotate_shortcut,
            &defaults.annotate_shortcut,
        ),
    ] {
        if let Err(error) = check_chord(shortcut) {
            repairs.retain(|repair| repair.field != field);
            repairs.push(Repair {
                field: field.into(),
                notice: format!(
                    "Your {name} shortcut, {}, is now {} because {}.",
                    spoken_chord(shortcut),
                    spoken_chord(default),
                    error.reason()
                ),
            });
            *shortcut = default.clone();
        }
    }
    if settings.annotate_chord().ok() == settings.chord().ok() {
        let taken = settings.annotate_shortcut.clone();
        if defaults.annotate_chord().ok() != settings.chord().ok() {
            settings.annotate_shortcut = defaults.annotate_shortcut.clone();
            repairs.push(Repair {
                field: "annotateShortcut".into(),
                notice: format!(
                    "Your Annotate shortcut, {}, is now {} because it was also your dictation shortcut.",
                    spoken_chord(&taken),
                    spoken_chord(&settings.annotate_shortcut)
                ),
            });
        } else if settings.annotate_enabled {
            settings.annotate_enabled = false;
            repairs.push(Repair {
                field: "annotateEnabled".into(),
                notice: format!(
                    "Annotate is off because its shortcut, {}, was also your dictation shortcut.",
                    spoken_chord(&taken)
                ),
            });
        }
    }
    Ok(Some((settings, repairs)))
}

fn field_label(field: &str) -> &str {
    match field {
        "shortcut" => "dictation shortcut",
        "shortcutMode" => "shortcut mode",
        "launchAtLogin" => "launch at login",
        "appearanceMode" => "appearance",
        "overlayPosition" => "overlay position",
        "historyRetention" => "history",
        "speechModel" => "speech model",
        "pasteMethod" => "paste",
        "restoreClipboard" => "restore clipboard",
        "trailingSpace" => "trailing space",
        "capitalizeFirst" => "capitalisation",
        "micMode" => "microphone mode",
        "soundFeedback" => "sound feedback",
        "debugRecordings" => "debug recordings",
        "annotateShortcut" => "Annotate shortcut",
        "annotateEnabled" => "Annotate",
        "annotateShowInRecordings" => "Annotate recording",
        other => other,
    }
}

/// The settings file as it reads, with every repair applied. Missing gives
/// the defaults.
#[cfg(test)]
pub fn load(path: &Path) -> Result<Settings, String> {
    Ok(read(path)?.map_or_else(Settings::default, |(settings, _)| settings))
}

fn migrate(settings: &mut Settings) {
    if settings.version < CURRENT_VERSION {
        settings.version = CURRENT_VERSION;
    }
}

/// Settings at launch, and what the launch may do with them.
pub struct Loaded {
    pub settings: Settings,
    /// True only when the file was read and its history setting was used as
    /// written. Startup prunes history only then.
    pub may_prune: bool,
    /// One sentence per setting that was reset, for the General page.
    pub notices: Vec<String>,
    pub problem: Option<Problem>,
}

fn backup_path(path: &Path) -> std::path::PathBuf {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    path.with_file_name(format!("settings.json.bad-{timestamp}"))
}

pub fn load_or_default(path: &Path) -> Loaded {
    match read(path) {
        Ok(None) => Loaded {
            settings: Settings::default(),
            may_prune: false,
            notices: Vec::new(),
            problem: None,
        },
        Ok(Some((settings, repairs))) => {
            if !repairs.is_empty() {
                let backup = backup_path(path);
                match fs::copy(path, &backup) {
                    Ok(_) => {
                        if let Err(error) = save(path, &settings) {
                            log::warn!("[settings] save repaired settings: {error}");
                        }
                    }
                    Err(error) => log::warn!(
                        "[settings] back up {} before repairing: {error}",
                        path.display()
                    ),
                }
                for repair in &repairs {
                    log::warn!("[settings] repaired {}: {}", repair.field, repair.notice);
                }
            }
            Loaded {
                may_prune: !repairs.iter().any(|r| r.field == "historyRetention"),
                notices: repairs.into_iter().map(|r| r.notice).collect(),
                settings,
                problem: None,
            }
        }
        Err(error) => {
            let backup = backup_path(path);
            let (fix, detail) = match fs::rename(path, &backup) {
                Ok(()) => (
                    "Using defaults, and keeping all your history. Your old file is saved beside them.",
                    format!("{error}; original saved to {}", backup.display()),
                ),
                Err(backup_error) => (
                    "Using defaults, and keeping all your history. Choose your preferences again in Settings.",
                    format!("{error}; could not back up original: {backup_error}"),
                ),
            };
            Loaded {
                settings: Settings {
                    history_retention: UNREADABLE_RETENTION,
                    ..Settings::default()
                },
                may_prune: false,
                notices: Vec::new(),
                problem: Some(Problem::new("Settings couldn’t be read", fix).because(detail)),
            }
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
    // Keys this build retired are written back without them: the single theme
    // id the theme migration replaced, and the translucent window that was
    // removed. A load ignores them; the next save drops them.
    for retired in ["theme", "glass"] {
        value.remove(retired);
    }
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
                theme_light: Theme::Paper,
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
            theme_light: Theme::Paper,
            theme_dark: Theme::Capy,
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
    fn a_v1_file_with_one_theme_gives_both_appearances_that_theme() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        // Written the way the shipped build wrote a file: one theme id for both
        // appearances, version 1, and the rest of the person's settings.
        fs::write(
            &path,
            r#"{
  "version": 1,
  "shortcut": "Control+Shift+KeyD",
  "shortcutMode": "toggle",
  "microphone": "External microphone",
  "launchAtLogin": true,
  "theme": "rosePine",
  "appearanceMode": "dark",
  "overlayPosition": "top",
  "historyRetention": "forever"
}"#,
        )
        .unwrap();
        let settings = load(&path).unwrap();
        assert_eq!(settings.theme_light, Theme::RosePine);
        assert_eq!(settings.theme_dark, Theme::RosePine);
        assert_eq!(settings.version, 2);
        assert_eq!(settings.shortcut, "Control+Shift+KeyD");
        assert_eq!(settings.shortcut_mode, ShortcutMode::Toggle);
        assert_eq!(settings.microphone.as_deref(), Some("External microphone"));
        assert!(settings.launch_at_login);
        assert_eq!(settings.appearance_mode, AppearanceMode::Dark);
        assert_eq!(settings.overlay_position, OverlayPosition::Top);
        assert_eq!(settings.history_retention, HistoryRetention::Forever);
        save(&path, &settings).unwrap();
        let written: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(written["themeLight"], "rosePine");
        assert_eq!(written["themeDark"], "rosePine");
        assert!(written.get("theme").is_none(), "{written}");
    }

    #[test]
    fn a_file_that_already_separates_the_modes_is_not_rewritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{"version":2,"themeLight":"paper","themeDark":"capy","theme":"ember"}"#,
        )
        .unwrap();
        let settings = load(&path).unwrap();
        assert_eq!(settings.theme_light, Theme::Paper);
        assert_eq!(settings.theme_dark, Theme::Capy);
    }

    #[test]
    fn a_file_with_one_new_key_keeps_it_and_defaults_the_other() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"version":2,"themeDark":"neutral"}"#).unwrap();
        let settings = load(&path).unwrap();
        assert_eq!(settings.theme_dark, Theme::Neutral);
        assert_eq!(settings.theme_light, Theme::Ember);
    }

    #[test]
    fn an_unknown_legacy_theme_falls_back_in_both_modes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{"version":1,"theme":"dracula","shortcut":"Control+KeyD"}"#,
        )
        .unwrap();
        let Loaded {
            settings, problem, ..
        } = load_or_default(&path);
        assert!(problem.is_none());
        assert_eq!(settings.theme_light, Theme::Ember);
        assert_eq!(settings.theme_dark, Theme::Ember);
        assert_eq!(settings.shortcut, "Control+KeyD");
    }

    #[test]
    fn the_theme_for_an_appearance_is_the_one_for_that_side() {
        let settings = Settings {
            theme_light: Theme::Paper,
            theme_dark: Theme::Neutral,
            ..Settings::default()
        };
        assert_eq!(settings.theme_for(false), Theme::Paper);
        assert_eq!(settings.theme_for(true), Theme::Neutral);
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
                theme_light: theme,
                theme_dark: theme,
                ..Settings::default()
            };
            save(&path, &settings).unwrap();
            let value: serde_json::Value =
                serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            assert_eq!(value["themeLight"], id);
            assert_eq!(value["themeDark"], id);
            assert_eq!(load(&path).unwrap(), settings);
        }
    }

    #[test]
    fn unknown_theme_falls_back_to_ember_and_keeps_other_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        for theme in [
            r#""dracula""#,
            r#""Ember""#,
            "42",
            "null",
            r#"{"id":"capy"}"#,
        ] {
            fs::write(
                &path,
                format!(r#"{{"shortcut":"Control+KeyD","theme":{theme},"appearanceMode":"dark"}}"#),
            )
            .unwrap();
            let Loaded {
                settings, problem, ..
            } = load_or_default(&path);
            assert!(problem.is_none(), "{theme}");
            assert_eq!(settings.theme_light, Theme::Ember, "{theme}");
            assert_eq!(settings.theme_dark, Theme::Ember, "{theme}");
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
        assert_eq!(settings.version, 2);
        assert_eq!(settings.shortcut_mode, ShortcutMode::Hold);
        assert_eq!(settings.microphone, None);
        assert!(!settings.launch_at_login);
        assert_eq!(settings.theme_light, Theme::Ember);
        assert_eq!(settings.theme_dark, Theme::Ember);
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
        fs::write(&path, r#"{"version":3,"newSetting":true}"#).unwrap();
        let settings = load(&path).unwrap();
        assert_eq!(settings.version, 3);
        assert_eq!(settings.shortcut, "Alt+Shift+Space");
        let updated = Settings {
            launch_at_login: true,
            ..settings
        };
        save(&path, &updated).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(value["newSetting"], true);
        assert_eq!(value["version"], 3);
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

    /// The translucent window was removed. A v2 file that still carries its
    /// key loads with every other choice intact, and the next save drops the
    /// key instead of carrying it forward.
    #[test]
    fn a_removed_glass_setting_loads_and_is_dropped_on_save() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{"version":2,"themeLight":"paper","themeDark":"capy","glass":true}"#,
        )
        .unwrap();
        let settings = load(&path).unwrap();
        assert_eq!(settings.theme_light, Theme::Paper);
        assert_eq!(settings.theme_dark, Theme::Capy);
        save(&path, &settings).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(value.get("glass"), None);
        assert_eq!(value["themeLight"], "paper");
        assert_eq!(load(&path).unwrap(), settings);
    }

    fn backups(dir: &Path) -> Vec<String> {
        fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap())
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("settings.json.bad-")
            })
            .map(|entry| fs::read_to_string(entry.path()).unwrap())
            .collect()
    }

    /// A file that isn't a JSON object is set aside whole. Nothing in it can
    /// be trusted, the history setting included, so history is kept and the
    /// launch must not prune.
    #[test]
    fn an_unreadable_file_is_backed_up_and_keeps_all_history() {
        for original in ["{broken", "[]", "null"] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("settings.json");
            fs::write(&path, original).unwrap();
            let loaded = load_or_default(&path);
            assert_eq!(
                loaded.settings,
                Settings {
                    history_retention: HistoryRetention::Forever,
                    ..Settings::default()
                }
            );
            assert!(!loaded.may_prune);
            let problem = loaded.problem.unwrap();
            assert_eq!(problem.title, "Settings couldn’t be read");
            assert!(problem.fix.contains("keeping all your history"));
            assert!(!path.exists());
            assert_eq!(backups(dir.path()), [original]);
        }
    }

    /// The DICT-14 repro: a "Forever" file from 0.1.2 whose dictation
    /// shortcut is now reserved used to load as defaults, so launch pruned to
    /// a week. Only the shortcut is reset now; everything else stays.
    #[test]
    fn a_reserved_dictation_chord_resets_only_that_shortcut() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let original = r#"{"version":2,"shortcut":"Super+Space","historyRetention":"forever","themeLight":"paper","themeDark":"capy","speechModel":"small.en","futureKey":1}"#;
        fs::write(&path, original).unwrap();
        let loaded = load_or_default(&path);
        assert!(loaded.problem.is_none());
        assert!(loaded.may_prune);
        assert_eq!(loaded.settings.shortcut, "Alt+Shift+Space");
        assert_eq!(loaded.settings.history_retention, HistoryRetention::Forever);
        assert_eq!(loaded.settings.theme_light, Theme::Paper);
        assert_eq!(loaded.settings.theme_dark, Theme::Capy);
        assert_eq!(loaded.settings.speech_model, SpeechModel::SmallEn);
        assert_eq!(
            loaded.notices,
            [
                "Your dictation shortcut, Command + Space, is now Option + Shift + Space because macOS reserves it."
            ]
        );
        assert_eq!(backups(dir.path()), [original]);
        let written: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(written["shortcut"], "Alt+Shift+Space");
        assert_eq!(written["historyRetention"], "forever");
        assert_eq!(written["futureKey"], 1);
        let again = load_or_default(&path);
        assert!(again.notices.is_empty(), "a repair is made once");
        assert_eq!(again.settings, loaded.settings);
    }

    #[test]
    fn an_unusable_annotate_chord_resets_and_annotate_can_still_be_turned_off() {
        for (stored, reason) in [
            ("Super+KeyH", "macOS reserves it"),
            ("KeyS", "it had no Command, Option, Control or Shift key"),
            ("not-a-key", "dictate couldn’t read it"),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("settings.json");
            fs::write(
                &path,
                format!(
                    r#"{{"version":2,"annotateShortcut":"{stored}","historyRetention":"month"}}"#
                ),
            )
            .unwrap();
            let loaded = load_or_default(&path);
            assert_eq!(loaded.settings.annotate_shortcut, "Alt+Shift+KeyA");
            assert_eq!(loaded.settings.history_retention, HistoryRetention::Month);
            assert!(loaded.may_prune);
            assert_eq!(loaded.notices.len(), 1);
            assert!(loaded.notices[0].starts_with("Your Annotate shortcut"));
            assert!(
                loaded.notices[0].ends_with(&format!("because {reason}.")),
                "{}",
                loaded.notices[0]
            );
        }
        let stuck = Settings {
            annotate_shortcut: "Super+KeyH".into(),
            ..Settings::default()
        };
        let off = Settings {
            annotate_enabled: false,
            ..stuck.clone()
        };
        // Nothing was registered for the unusable chord, so there is nothing
        // to unregister: the save goes through and changes no registration.
        assert_eq!(
            off.validate_changes(&stuck, || Ok(Vec::new()), |_| true)
                .unwrap()
                .annotate,
            None
        );
    }

    #[test]
    fn an_unreadable_history_setting_keeps_everything_and_blocks_the_prune() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{"version":2,"historyRetention":"decade","soundFeedback":true,"micMode":42}"#,
        )
        .unwrap();
        let loaded = load_or_default(&path);
        assert_eq!(loaded.settings.history_retention, HistoryRetention::Forever);
        assert!(loaded.settings.sound_feedback);
        assert_eq!(loaded.settings.mic_mode, MicMode::OnDemand);
        assert!(!loaded.may_prune);
        assert_eq!(loaded.notices.len(), 2, "{:?}", loaded.notices);
        assert!(
            loaded
                .notices
                .iter()
                .any(|n| n.contains("keeps every dictation"))
        );
    }

    #[test]
    fn an_annotate_chord_equal_to_the_dictation_one_is_reset() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{"shortcut":"Control+KeyD","annotateShortcut":"Control+KeyD"}"#,
        )
        .unwrap();
        let loaded = load_or_default(&path);
        assert_eq!(loaded.settings.shortcut, "Control+KeyD");
        assert_eq!(loaded.settings.annotate_shortcut, "Alt+Shift+KeyA");
        assert!(loaded.notices[0].contains("was also your dictation shortcut"));
    }

    #[test]
    fn macos_reserved_shortcuts_cannot_replace_either_binding() {
        for shortcut in [
            "Super+KeyQ",
            "Super+KeyH",
            "Super+Tab",
            "Super+Space",
            "Super+Alt+Escape",
        ] {
            assert!(
                parse_chord(shortcut)
                    .unwrap_err()
                    .contains("reserved by macOS")
            );
        }
        assert!(parse_chord("Super+Shift+KeyK").is_ok());
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
