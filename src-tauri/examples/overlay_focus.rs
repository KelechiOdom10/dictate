//! Spike S1 (Annotate risk R1): can a full-screen overlay take mouse drags
//! without taking focus from the app being annotated?
//!
//! One transparent Tauri webview window covers the main display. It is turned
//! into a non-activating `NSPanel` with `tauri-nspanel`, raised to screen-saver
//! level and put on every Space. Every 3 s it flips between click-through
//! (mouse goes to the app below) and pen (mouse goes to the overlay), for 60 s.
//!
//! A main-thread probe samples AppKit at 60 Hz and logs every change in:
//! `NSApp.isActive`, the frontmost app, whether the panel is the key window,
//! and the mouse button. While the left button is down it logs the pointer.
//! The page inside the overlay draws ink and reports each pointer event back
//! through `document.title`, so the log also shows whether the webview itself
//! received the drag.
//!
//! Run: cargo run --manifest-path src-tauri/Cargo.toml --example overlay_focus
//! Then type in TextEdit and drag across the screen until it exits.

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("overlay_focus is a macOS spike");
}

#[cfg(target_os = "macos")]
fn main() {
    spike::run();
}

#[cfg(target_os = "macos")]
mod spike {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{
        NSApplication, NSEvent, NSScreenSaverWindowLevel, NSWindowCollectionBehavior,
        NSWindowStyleMask, NSWorkspace,
    };
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};
    use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
    use tauri_nspanel::{ManagerExt, WebviewWindowExt};

    mod panel {
        tauri_nspanel::tauri_panel! {
            panel!(OverlayPanel {
                config: {
                    can_become_key_window: false,
                    can_become_main_window: false,
                    is_floating_panel: true
                }
            })
        }
    }
    use panel::OverlayPanel;

    const LABEL: &str = "overlay";
    const PHASE: Duration = Duration::from_secs(3);
    const RUN: Duration = Duration::from_secs(60);
    const PROBE: Duration = Duration::from_millis(16);

    const PAGE: &str = r#"<!doctype html><html><head><style>
html,body{margin:0;height:100%;background:transparent;overflow:hidden;cursor:crosshair}
body{box-shadow:inset 0 0 0 6px var(--edge,rgba(255,59,48,.0))}
#badge{position:fixed;top:40px;left:50%;transform:translateX(-50%);font:600 15px -apple-system;
color:#fff;background:rgba(0,0,0,.7);padding:6px 12px;border-radius:8px}
canvas{position:fixed;inset:0}
</style></head><body><div id="badge">overlay spike</div><canvas id="c"></canvas><script>
const c=document.getElementById('c'),g=c.getContext('2d'),dpr=devicePixelRatio;
function fit(){c.width=innerWidth*dpr;c.height=innerHeight*dpr;c.style.width=innerWidth+'px';c.style.height=innerHeight+'px';g.scale(dpr,dpr);}
fit();addEventListener('resize',fit);
let down=false,n=0;
function report(kind,e){n++;document.title=kind+' '+n+' '+Math.round(e.clientX)+' '+Math.round(e.clientY);}
c.addEventListener('pointerdown',e=>{down=true;g.beginPath();g.moveTo(e.clientX,e.clientY);report('down',e);});
c.addEventListener('pointermove',e=>{if(!down)return;g.lineTo(e.clientX,e.clientY);g.strokeStyle='#FF3B30';g.lineWidth=4;g.lineCap='round';g.stroke();report('move',e);});
addEventListener('pointerup',e=>{down=false;report('up',e);setTimeout(()=>g.clearRect(0,0,innerWidth,innerHeight),1500);});
window.setMode=(pen)=>{document.getElementById('badge').textContent=pen?'PEN: drag here, then keep typing':'CLICK-THROUGH: click and type in TextEdit';
document.body.style.setProperty('--edge',pen?'rgba(255,59,48,.8)':'rgba(52,199,89,.8)');};
</script></body></html>"#;

    #[derive(Default)]
    struct Phase {
        pen: bool,
        active_samples: u32,
        key_samples: u32,
        mouse_down_samples: u32,
        native_drag_points: u32,
        webview_events: u32,
        frontmost: Vec<String>,
    }

    #[derive(Default)]
    struct Log {
        start: Option<Instant>,
        phases: Vec<Phase>,
        last_line: String,
        samples: u32,
    }

    fn ms(log: &Log) -> u128 {
        log.start.map_or(0, |s| s.elapsed().as_millis())
    }

    pub fn run() {
        let log = Arc::new(Mutex::new(Log::default()));
        let mut context = tauri::generate_context!();
        context.config_mut().app.windows.clear();

        let setup_log = log.clone();
        tauri::Builder::default()
            // Tauri activates the app at launch by default, which takes focus
            // from whatever the user was typing in. Run 1 measured exactly that.
            .activate_ignoring_other_apps(std::env::var_os("SPIKE_DEFAULT_ACTIVATION").is_some())
            .plugin(tauri_nspanel::init())
            .register_uri_scheme_protocol("spike", |_, _| {
                tauri::http::Response::builder()
                    .header("Content-Type", "text/html; charset=utf-8")
                    .body(PAGE.as_bytes().to_vec())
                    .unwrap()
            })
            .setup(move |app| {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                build_overlay(app.handle(), setup_log.clone())?;
                start_threads(app.handle().clone(), setup_log.clone());
                Ok(())
            })
            .run(context)
            .expect("overlay spike failed");
    }

    fn build_overlay(app: &AppHandle, log: Arc<Mutex<Log>>) -> tauri::Result<()> {
        let monitor = app
            .primary_monitor()?
            .expect("no primary monitor");
        let scale = monitor.scale_factor();
        let size = monitor.size().to_logical::<f64>(scale);
        let position = monitor.position().to_logical::<f64>(scale);
        let url = WebviewUrl::CustomProtocol("spike://localhost/".parse().unwrap());

        let window = WebviewWindowBuilder::new(app, LABEL, url)
            .title("overlay spike")
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .resizable(false)
            .skip_taskbar(true)
            .focused(false)
            .focusable(false)
            .accept_first_mouse(true)
            .visible(false)
            .position(position.x, position.y)
            .inner_size(size.width, size.height)
            .on_document_title_changed(move |_, title| {
                let mut log = log.lock().unwrap();
                let t = ms(&log);
                if let Some(phase) = log.phases.last_mut() {
                    phase.webview_events += 1;
                }
                println!("[{t:>6} ms] webview {title}");
            })
            .build()?;

        clear_webview_background(&window);

        let panel = window.to_panel::<OverlayPanel>()?;
        panel
            .add_style_mask(NSWindowStyleMask::NonactivatingPanel)
            .expect("nonactivating style mask rejected");
        panel.set_level(NSScreenSaverWindowLevel as i64);
        panel.set_collection_behavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::Stationary,
        );
        panel.set_hides_on_deactivate(false);
        panel.set_transparent(true);
        panel.set_has_shadow(false);
        window.set_ignore_cursor_events(true)?;
        panel.order_front_regardless();

        println!(
            "overlay {}x{} pt at ({}, {}), level {}, style {:?}, can_become_key {}",
            size.width,
            size.height,
            position.x,
            position.y,
            NSScreenSaverWindowLevel,
            panel.as_panel().styleMask(),
            panel.can_become_key_window(),
        );
        Ok(())
    }

    /// The webview paints white unless told not to; Tauri only exposes this
    /// behind the `macos-private-api` feature, so set the same KVC key here.
    fn clear_webview_background(window: &tauri::WebviewWindow) {
        window
            .with_webview(|webview| unsafe {
                let view = webview.inner() as *mut objc2::runtime::AnyObject;
                let no = objc2_foundation::NSNumber::new_bool(false);
                let key = objc2_foundation::NSString::from_str("drawsBackground");
                let _: () = objc2::msg_send![&*view, setValue: &*no, forKey: &*key];
            })
            .expect("could not reach the webview");
    }

    fn start_threads(app: AppHandle, log: Arc<Mutex<Log>>) {
        log.lock().unwrap().start = Some(Instant::now());

        let toggler = app.clone();
        let toggle_log = log.clone();
        thread::spawn(move || {
            let phases = (RUN.as_secs() / PHASE.as_secs()) as usize;
            for index in 0..phases {
                let pen = index % 2 == 1;
                {
                    let mut log = toggle_log.lock().unwrap();
                    let t = ms(&log);
                    log.phases.push(Phase { pen, ..Phase::default() });
                    println!(
                        "[{t:>6} ms] phase {:>2}: {}",
                        index + 1,
                        if pen { "PEN (overlay takes the mouse)" } else { "CLICK-THROUGH" }
                    );
                }
                let window = toggler.get_webview_window(LABEL).unwrap();
                window.set_ignore_cursor_events(!pen).unwrap();
                let _ = window.eval(format!("setMode({pen})"));
                thread::sleep(PHASE);
            }
            let handle = toggler.clone();
            let _ = toggler.run_on_main_thread(move || {
                if let Ok(window) = handle.get_webview_window(LABEL).ok_or(()) {
                    let _ = window.set_ignore_cursor_events(true);
                }
                if let Ok(panel) = handle.get_webview_panel(LABEL) {
                    panel.hide();
                }
            });
            thread::sleep(Duration::from_millis(200));
            report(&toggle_log.lock().unwrap());
            toggler.exit(0);
        });

        thread::spawn(move || loop {
            let probe_log = log.clone();
            let handle = app.clone();
            let _ = app.run_on_main_thread(move || probe(&handle, &probe_log));
            thread::sleep(PROBE);
        });
    }

    fn probe(app: &AppHandle, log: &Mutex<Log>) {
        let mtm = MainThreadMarker::new().expect("probe must run on the main thread");
        let ns_app = NSApplication::sharedApplication(mtm);
        let active = ns_app.isActive();
        let frontmost = NSWorkspace::sharedWorkspace()
            .frontmostApplication()
            .and_then(|a| a.localizedName())
            .map_or_else(|| "?".into(), |n| n.to_string());
        let panel_key = app
            .get_webview_panel(LABEL)
            .map(|p| p.as_panel().isKeyWindow())
            .unwrap_or(false);
        let buttons = NSEvent::pressedMouseButtons();
        let point = NSEvent::mouseLocation();
        let left_down = buttons & 1 == 1;

        let mut log = log.lock().unwrap();
        log.samples += 1;
        let t = ms(&log);
        let Some(phase) = log.phases.last_mut() else { return };
        let pen = phase.pen;
        if active {
            phase.active_samples += 1;
        }
        if panel_key {
            phase.key_samples += 1;
        }
        if !phase.frontmost.contains(&frontmost) {
            phase.frontmost.push(frontmost.clone());
        }
        if left_down {
            phase.mouse_down_samples += 1;
            if pen {
                phase.native_drag_points += 1;
                println!("[{t:>6} ms] drag point ({:.0}, {:.0}) isActive={active}", point.x, point.y);
            }
        }
        let line = format!(
            "isActive={active} frontmost={frontmost} panelKey={panel_key} leftDown={left_down}"
        );
        if line != log.last_line {
            println!("[{t:>6} ms] {line}");
            log.last_line = line;
        }
    }

    fn report(log: &Log) {
        println!("\n=== overlay_focus summary ({} probe samples) ===", log.samples);
        println!("phase mode           isActive key mouseDown dragPts webview frontmost");
        for (i, p) in log.phases.iter().enumerate() {
            println!(
                "{:>5} {:<14} {:>8} {:>3} {:>9} {:>7} {:>7} {}",
                i + 1,
                if p.pen { "pen" } else { "click-through" },
                p.active_samples,
                p.key_samples,
                p.mouse_down_samples,
                p.native_drag_points,
                p.webview_events,
                p.frontmost.join(","),
            );
        }
        let active: u32 = log.phases.iter().map(|p| p.active_samples).sum();
        let key: u32 = log.phases.iter().map(|p| p.key_samples).sum();
        let pen_events: u32 = log.phases.iter().filter(|p| p.pen).map(|p| p.webview_events).sum();
        let leaked: u32 = log.phases.iter().filter(|p| !p.pen).map(|p| p.webview_events).sum();
        let dictate_front = log
            .phases
            .iter()
            .any(|p| p.frontmost.iter().any(|f| f.contains("dictate") || f.contains("overlay_focus")));
        println!("isActive samples: {active} (pass: 0)");
        println!("panel key samples: {key} (pass: 0)");
        println!("spike ever frontmost: {dictate_front} (pass: false)");
        println!("webview pointer events in pen phases: {pen_events} (pass: > 0 if you dragged)");
        println!("webview pointer events in click-through phases: {leaked} (pass: 0)");
        let pass = active == 0 && key == 0 && !dictate_front && leaked == 0;
        println!("R1 automatic checks: {}", if pass { "PASS" } else { "FAIL" });
        println!("Now check TextEdit by eye: every character you typed is there.");
    }
}
