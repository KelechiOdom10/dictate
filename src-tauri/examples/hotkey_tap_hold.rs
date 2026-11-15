//! Spike S2 (Annotate risk R2): does ⌥⇧S give clean Pressed/Released pairs,
//! so one shortcut can mean "tap to start or stop" and "hold to draw"?
//!
//! Registers ⌥⇧S through `tauri-plugin-global-shortcut`, the same plugin the
//! dictation chord uses, and logs every event with its time and the pointer
//! position. Each Released is classified with the design's rule: held under
//! 250 ms and pointer moved under 6 pt is a tap, anything else is a hold.
//!
//! The run expects 20 taps, then 10 holds of about 2 s. Pressed events that
//! arrive while the key is already down are counted as auto-repeat. After 30
//! releases (or 3 minutes) it prints the confusion counts and exits.
//!
//! Run: cargo run --manifest-path src-tauri/Cargo.toml --example hotkey_tap_hold

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

const TAP_MAX: Duration = Duration::from_millis(250);
const TAP_MAX_MOVE_PT: f64 = 6.0;
const TAPS: usize = 20;
const HOLDS: usize = 10;
const TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Tap,
    Hold,
}

#[derive(Default)]
struct Run {
    start: Option<Instant>,
    open: Option<(Instant, (f64, f64))>,
    pressed: usize,
    released: usize,
    repeat_pressed: usize,
    orphan_released: usize,
    results: Vec<(Kind, Kind, u128, f64)>,
}

fn classify(held: Duration, moved_pt: f64) -> Kind {
    if held < TAP_MAX && moved_pt < TAP_MAX_MOVE_PT {
        Kind::Tap
    } else {
        Kind::Hold
    }
}

#[cfg(target_os = "macos")]
fn pointer() -> (f64, f64) {
    use core_graphics::event::CGEvent;
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .and_then(CGEvent::new)
        .map(|e| {
            let p = e.location();
            (p.x, p.y)
        })
        .unwrap_or((f64::NAN, f64::NAN))
}

#[cfg(not(target_os = "macos"))]
fn pointer() -> (f64, f64) {
    (f64::NAN, f64::NAN)
}

fn expected(index: usize) -> Kind {
    if index < TAPS { Kind::Tap } else { Kind::Hold }
}

fn prompt(done: usize) {
    if done < TAPS {
        println!("  -> tap ⌥⇧S quickly ({}/{TAPS} taps)", done + 1);
    } else if done < TAPS + HOLDS {
        println!(
            "  -> hold ⌥⇧S for about 2 s, then let go ({}/{HOLDS} holds)",
            done - TAPS + 1
        );
    }
}

fn report(run: &Run) {
    let mut matrix = [[0usize; 2]; 2];
    for (want, got, _, _) in &run.results {
        matrix[*want as usize][*got as usize] += 1;
    }
    println!("\n=== hotkey_tap_hold summary ===");
    println!("Pressed {} / Released {}", run.pressed, run.released);
    println!("extra Pressed while held (auto-repeat): {}", run.repeat_pressed);
    println!("Released with no open Pressed: {}", run.orphan_released);
    println!("                 classified tap  classified hold");
    println!("intended tap     {:>14}  {:>15}", matrix[0][0], matrix[0][1]);
    println!("intended hold    {:>14}  {:>15}", matrix[1][0], matrix[1][1]);
    let taps: Vec<_> = run.results.iter().filter(|r| r.0 == Kind::Tap).map(|r| r.2).collect();
    let holds: Vec<_> = run.results.iter().filter(|r| r.0 == Kind::Hold).map(|r| r.2).collect();
    let range = |v: &[u128]| match (v.iter().min(), v.iter().max()) {
        (Some(a), Some(b)) => format!("{a}–{b} ms"),
        _ => "n/a".into(),
    };
    println!("tap held: {}   hold held: {}", range(&taps), range(&holds));
    let correct = matrix[0][0] + matrix[1][1];
    let pass = correct == run.results.len()
        && run.results.len() == TAPS + HOLDS
        && run.repeat_pressed == 0
        && run.orphan_released == 0;
    println!(
        "R2: {correct}/{} correct -> {}",
        run.results.len(),
        if pass { "PASS" } else { "FAIL" }
    );
}

fn main() {
    let run = Arc::new(Mutex::new(Run::default()));
    let mut context = tauri::generate_context!();
    context.config_mut().app.windows.clear();
    let chord = Shortcut::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyS);

    let handler_run = run.clone();
    let timeout_run = run.clone();
    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, _shortcut, event| {
                    let now = Instant::now();
                    let at = pointer();
                    let mut run = handler_run.lock().unwrap();
                    let t = now.duration_since(run.start.unwrap_or(now)).as_millis();
                    match event.state() {
                        ShortcutState::Pressed => {
                            run.pressed += 1;
                            if run.open.is_some() {
                                run.repeat_pressed += 1;
                                println!("[{t:>7} ms] Pressed  (REPEAT while held, ignored)");
                            } else {
                                run.open = Some((now, at));
                                println!("[{t:>7} ms] Pressed  at ({:.1}, {:.1})", at.0, at.1);
                            }
                        }
                        ShortcutState::Released => {
                            run.released += 1;
                            let Some((down, from)) = run.open.take() else {
                                run.orphan_released += 1;
                                println!("[{t:>7} ms] Released (no open Pressed)");
                                return;
                            };
                            let held = now.duration_since(down);
                            let moved = ((at.0 - from.0).powi(2) + (at.1 - from.1).powi(2)).sqrt();
                            let got = classify(held, moved);
                            let index = run.results.len();
                            let want = expected(index);
                            run.results.push((want, got, held.as_millis(), moved));
                            println!(
                                "[{t:>7} ms] Released held={} ms moved={moved:.1} pt -> {got:?} (intended {want:?}){}",
                                held.as_millis(),
                                if got == want { "" } else { "  MISMATCH" }
                            );
                            prompt(run.results.len());
                            if run.results.len() == TAPS + HOLDS {
                                report(&run);
                                app.exit(0);
                            }
                        }
                    }
                })
                .build(),
        )
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            app.global_shortcut()
                .register(chord)
                .expect("could not register ⌥⇧S; is another app using it?");
            run.lock().unwrap().start = Some(Instant::now());
            println!("⌥⇧S registered. Focus any text field so stray letters would show up.");
            prompt(0);

            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(TIMEOUT);
                println!("\ntimed out after {} s", TIMEOUT.as_secs());
                report(&timeout_run.lock().unwrap());
                handle.exit(1);
            });
            Ok(())
        })
        .run(context)
        .expect("hotkey spike failed");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_needs_both_short_and_still_for_a_tap() {
        assert_eq!(classify(Duration::from_millis(120), 0.0), Kind::Tap);
        assert_eq!(classify(Duration::from_millis(249), 5.9), Kind::Tap);
        assert_eq!(classify(Duration::from_millis(250), 0.0), Kind::Hold);
        assert_eq!(classify(Duration::from_millis(100), 6.0), Kind::Hold);
        assert_eq!(classify(Duration::from_secs(2), 0.0), Kind::Hold);
    }
}
