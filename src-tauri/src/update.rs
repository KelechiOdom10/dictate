//! Automatic updates: check, download, verify, install, relaunch.
//!
//! The updater plugin does the checking and the signature verification (the
//! public key and endpoint are in `tauri.conf.json`). This module decides
//! *when*: a check runs shortly after launch and on request, the download
//! happens at once, and the install waits until neither dictation nor
//! Annotate is in use. The wait holds nothing: a take that starts first simply
//! pushes the install back.
//!
//! It fails soft. A launch check that cannot reach the endpoint only logs;
//! only a check the user asked for reports the error.

use crate::{Phase, Runtime, annotate};
use serde::Serialize;
use std::{sync::Mutex, time::Duration};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

const LAUNCH_DELAY: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const IDLE_POLL: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UpdateState {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Downloading {
        version: String,
    },
    /// Downloaded and verified; installs when dictation and Annotate are idle.
    Waiting {
        version: String,
    },
    Installing {
        version: String,
    },
    Failed {
        message: String,
    },
}

impl UpdateState {
    fn busy(&self) -> bool {
        matches!(
            self,
            Self::Checking
                | Self::Downloading { .. }
                | Self::Waiting { .. }
                | Self::Installing { .. }
        )
    }
}

/// Installing restarts the app, so it only happens when no take is recording
/// or being transcribed and no Annotate session is starting, recording or
/// packaging.
pub fn may_install(dictation: Phase, annotate: annotate::Phase) -> bool {
    dictation == Phase::Idle && annotate == annotate::Phase::Idle
}

/// Polls until `attempt` returns a value, sleeping `every` between tries.
fn when_ready<T>(mut attempt: impl FnMut() -> Option<T>, every: Duration) -> T {
    loop {
        if let Some(value) = attempt() {
            return value;
        }
        std::thread::sleep(every);
    }
}

pub fn status(app: &AppHandle) -> UpdateState {
    app.state::<Mutex<UpdateState>>()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

fn set(app: &AppHandle, next: UpdateState) {
    *app.state::<Mutex<UpdateState>>()
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = next.clone();
    if let Err(error) = app.emit("update-state", next) {
        log::warn!("[update] {error}");
    }
}

/// Check once shortly after launch. Release builds only: a dev build running
/// `pnpm tauri dev` must never replace itself.
pub fn check_on_launch(app: &AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tauri::async_runtime::spawn_blocking(|| std::thread::sleep(LAUNCH_DELAY))
            .await
            .ok();
        check(app, false).await;
    });
}

pub async fn check(app: AppHandle, manual: bool) {
    if cfg!(debug_assertions) {
        if manual {
            set(
                &app,
                UpdateState::Failed {
                    message: "Updates are off in development builds.".into(),
                },
            );
        }
        return;
    }
    {
        let state = app.state::<Mutex<UpdateState>>();
        let mut state = state.lock().unwrap_or_else(|e| e.into_inner());
        if state.busy() {
            return;
        }
        *state = UpdateState::Checking;
    }
    if manual {
        set(&app, UpdateState::Checking);
    }
    if let Err(error) = run(&app).await {
        log::warn!("[update] {error}");
        set(
            &app,
            if manual {
                UpdateState::Failed {
                    message: format!("Couldn't check for updates: {error}"),
                }
            } else {
                UpdateState::Idle
            },
        );
    }
}

async fn run(app: &AppHandle) -> Result<(), String> {
    let updater = app
        .updater_builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| e.to_string())?;
    let Some(update) = updater.check().await.map_err(|e| e.to_string())? else {
        set(app, UpdateState::UpToDate);
        return Ok(());
    };
    let version = update.version.clone();
    log::info!("[update] {version} available, downloading");
    set(
        app,
        UpdateState::Downloading {
            version: version.clone(),
        },
    );
    let bytes = update
        .download(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    log::info!("[update] {version} downloaded and verified; waiting for idle");
    set(
        app,
        UpdateState::Waiting {
            version: version.clone(),
        },
    );

    let handle = app.clone();
    let installed = tauri::async_runtime::spawn_blocking(move || {
        when_ready(
            || {
                // Holding the runtime lock through the install keeps a
                // shortcut press from starting a take in the gap.
                let state = handle.state::<Mutex<Runtime>>();
                let runtime = state.lock().ok()?;
                if !may_install(runtime.phase, runtime.annotate) {
                    return None;
                }
                set(
                    &handle,
                    UpdateState::Installing {
                        version: version.clone(),
                    },
                );
                let result = update.install(&bytes).map_err(|e| e.to_string());
                if result.is_ok() {
                    handle.restart();
                }
                Some(result)
            },
            IDLE_POLL,
        )
    })
    .await
    .map_err(|e| e.to_string())?;
    installed
}

#[cfg(test)]
mod tests {
    use super::*;
    use annotate::Phase as Annotate;

    #[test]
    fn installs_only_when_dictation_and_annotate_are_both_idle() {
        assert!(may_install(Phase::Idle, Annotate::Idle));
        for dictation in [Phase::Recording, Phase::Transcribing] {
            assert!(!may_install(dictation, Annotate::Idle));
        }
        for annotating in [Annotate::Starting, Annotate::Recording, Annotate::Packaging] {
            assert!(!may_install(Phase::Idle, annotating));
            assert!(!may_install(Phase::Recording, annotating));
        }
    }

    #[test]
    fn waits_through_a_busy_stretch_then_runs_exactly_once() {
        let mut busy_for = 3;
        let mut tries = 0;
        let mut ran = 0;
        let value = when_ready(
            || {
                tries += 1;
                if busy_for > 0 {
                    busy_for -= 1;
                    return None;
                }
                ran += 1;
                Some("installed")
            },
            Duration::ZERO,
        );
        assert_eq!((value, tries, ran), ("installed", 4, 1));
    }

    #[test]
    fn a_failed_or_idle_state_does_not_block_the_next_check() {
        assert!(!UpdateState::Idle.busy());
        assert!(!UpdateState::UpToDate.busy());
        assert!(
            !UpdateState::Failed {
                message: "x".into()
            }
            .busy()
        );
        assert!(UpdateState::Checking.busy());
        assert!(
            UpdateState::Waiting {
                version: "1".into()
            }
            .busy()
        );
    }

    #[test]
    fn states_serialise_with_a_kind_tag_the_ui_reads() {
        let json = serde_json::to_value(UpdateState::Waiting {
            version: "0.2.0".into(),
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({"kind": "waiting", "version": "0.2.0"})
        );
        assert_eq!(
            serde_json::to_value(UpdateState::UpToDate).unwrap(),
            serde_json::json!({"kind": "upToDate"})
        );
    }
}
