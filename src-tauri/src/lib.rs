mod bus;
mod capture;
mod claude;
mod ghostty;
mod jev;
mod state;
mod tmux;
mod tray;

use bus::{Bus, BusSlot, ChunkMsg};
use claude::ClaudeSession;
use jev::{Classifier, Job, Level, VerdictMsg};
use state::{Store, Summary};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State, Window, Wry};
use tauri_plugin_notification::NotificationExt;
use tmux::{AttachInfo, PaneInfo, TapManager};

struct AppState {
    bus: Arc<BusSlot>,
    taps: Arc<TapManager>,
    classifier: Arc<Classifier>,
    panes: Arc<Mutex<Vec<PaneInfo>>>,
    store: Arc<Store>,
    subs: Arc<PanelSubs>,
}

/// The board window: wants everything, terminal bytes included.
struct FullBus {
    chunk: Channel<ChunkMsg>,
    panes: Channel<Vec<PaneInfo>>,
    claude: Channel<Vec<ClaudeSession>>,
    verdict: Channel<VerdictMsg>,
}

impl Bus for FullBus {
    fn chunk(&self, msg: &ChunkMsg) -> bool {
        self.chunk.send(msg.clone()).is_ok()
    }
    fn panes(&self, panes: &[PaneInfo]) -> bool {
        self.panes.send(panes.to_vec()).is_ok()
    }
    fn claude(&self, sessions: &[ClaudeSession]) -> bool {
        self.claude.send(sessions.to_vec()).is_ok()
    }
    fn verdict(&self, verdict: &VerdictMsg) -> bool {
        self.verdict.send(verdict.clone()).is_ok()
    }
}

/// Menu bar panel subscribers: one channel each, carrying the whole summary.
#[derive(Default)]
struct PanelSubs(Mutex<HashMap<String, Channel<Summary>>>);

impl PanelSubs {
    fn add(&self, label: &str, ch: Channel<Summary>) {
        self.0.lock().expect("subs").insert(label.to_string(), ch);
    }
    fn broadcast(&self, s: &Summary) {
        self.0.lock().expect("subs").retain(|_, ch| ch.send(s.clone()).is_ok());
    }
}

#[tauri::command]
fn subscribe(
    window: Window,
    state: State<AppState>,
    on_chunk: Channel<ChunkMsg>,
    on_panes: Channel<Vec<PaneInfo>>,
    on_claude: Channel<Vec<ClaudeSession>>,
    on_verdict: Channel<VerdictMsg>,
) {
    let full = Arc::new(FullBus { chunk: on_chunk, panes: on_panes, claude: on_claude, verdict: on_verdict });
    // Prime the new subscriber so the board fills without waiting for a change.
    full.panes(&state.panes.lock().expect("panes"));
    full.claude(&state.store.sessions());
    for v in state.store.verdicts() {
        full.verdict(&v);
    }
    state.bus.set(window.label(), full);
}

#[tauri::command]
fn subscribe_summary(window: Window, state: State<AppState>, on_summary: Channel<Summary>) {
    let _ = on_summary.send(state.store.summary());
    state.subs.add(window.label(), on_summary);
}

#[tauri::command]
fn summary(state: State<AppState>) -> Summary {
    state.store.summary()
}

#[tauri::command]
fn tmux_available() -> bool {
    tmux::available()
}

#[tauri::command]
fn tmux_status() -> tmux::TmuxStatus {
    tmux::status()
}

#[tauri::command]
fn list_panes() -> Result<Vec<PaneInfo>, String> {
    tmux::list_panes()
}

/// Returns the pane's screen; the tap itself belongs to the poller.
#[tauri::command]
fn attach_pane(state: State<AppState>, id: String) -> Result<AttachInfo, String> {
    let pane = state
        .panes
        .lock()
        .expect("panes")
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .or_else(|| tmux::list_panes().ok()?.into_iter().find(|p| p.id == id))
        .ok_or_else(|| format!("no tmux pane {id}"))?;
    state.taps.attach(&pane)
}

/// Taps live as long as the pane does; a tile going away changes nothing.
#[tauri::command]
fn detach_pane(_id: String) -> bool {
    false
}

#[tauri::command]
fn focus_pane(app: AppHandle, state: State<AppState>, id: String) -> Result<(), String> {
    let pane = state
        .panes
        .lock()
        .expect("panes")
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or_else(|| format!("no tmux pane {id}"))?;
    tray::hide_panel(&app);
    tmux::focus(&pane)
}

#[tauri::command]
fn send_keys(id: String, keys: Vec<String>) -> Result<(), String> {
    tmux::send_keys(&id, &keys)
}

#[tauri::command]
fn send_line(id: String, text: String) -> Result<(), String> {
    tmux::send_line(&id, &text)
}

#[tauri::command]
fn new_window(session: String, name: String, command: String) -> Result<String, String> {
    tmux::new_window(&session, &name, &command)
}

#[tauri::command]
fn claude_sessions() -> Vec<ClaudeSession> {
    claude::sessions()
}

#[tauri::command]
fn ghostty_appearance() -> ghostty::Appearance {
    ghostty::load()
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct JevStatus {
    enabled: bool,
    reason: String,
}

#[tauri::command]
fn jev_status(state: State<AppState>) -> JevStatus {
    if state.classifier.enabled {
        JevStatus { enabled: true, reason: "key found".into() }
    } else {
        JevStatus { enabled: false, reason: "no key in TYPESAFE_API_KEY or ~/.config/typesafe/key".into() }
    }
}

#[tauri::command]
fn open_main(app: AppHandle) {
    tray::open_main(&app);
}

#[tauri::command]
fn hide_panel(app: AppHandle) {
    tray::hide_panel(&app);
}

#[tauri::command]
fn verdicts_path(state: State<AppState>) -> String {
    state.store.path().display().to_string()
}

const NOTIFY_RATE: Duration = Duration::from_secs(10);

/// A new summary: the panel hears it, the menu bar item recolours, and a
/// rising verdict becomes a notification unless the board is in front.
fn publish(app: &AppHandle<Wry>, subs: &PanelSubs, s: &Summary, note: Option<(String, String)>) {
    subs.broadcast(s);
    let needs_you = s.counts.get("attention").copied().unwrap_or(0) + s.counts.get("failing").copied().unwrap_or(0);
    tray::update(app, s.top, needs_you);
    if let Some((title, body)) = note {
        let app = app.clone();
        let _ = app.clone().run_on_main_thread(move || {
            let looking = app.get_webview_window(tray::MAIN).and_then(|w| w.is_focused().ok()).unwrap_or(false);
            if !looking {
                if let Err(e) = app.notification().builder().title(title).body(body).show() {
                    tmux::log(&format!("notify: {e}"));
                }
            }
        });
    }
}

/// The in-process subscriber behind the store, the verdicts file, the menu
/// bar item, the panel, and notifications. It runs with no window open.
struct ServiceBus {
    store: Arc<Store>,
    subs: Arc<PanelSubs>,
    app: AppHandle<Wry>,
    last_notified: Mutex<HashMap<String, Instant>>,
}

impl Bus for ServiceBus {
    fn chunk(&self, _: &ChunkMsg) -> bool {
        true
    }
    fn panes(&self, panes: &[PaneInfo]) -> bool {
        if let Some(s) = self.store.set_panes(panes.to_vec()) {
            publish(&self.app, &self.subs, &s, None);
        }
        true
    }
    fn claude(&self, sessions: &[ClaudeSession]) -> bool {
        if let Some(s) = self.store.set_sessions(sessions.to_vec()) {
            publish(&self.app, &self.subs, &s, None);
        }
        true
    }
    fn verdict(&self, v: &VerdictMsg) -> bool {
        let previous = self.store.previous_level(&v.id);
        let rising = v.level >= Level::Failing && previous.is_none_or(|p| v.level > p);
        if let Some(s) = self.store.set_verdict(v.clone()) {
            let mut note = None;
            if rising {
                let mut last = self.last_notified.lock().expect("notify");
                let now = Instant::now();
                if last.get(&v.id).is_none_or(|t| now.duration_since(*t) >= NOTIFY_RATE) {
                    last.insert(v.id.clone(), now);
                    note = Some(describe(&s, v));
                }
            }
            publish(&self.app, &self.subs, &s, note);
        }
        true
    }
}

fn describe(s: &Summary, v: &VerdictMsg) -> (String, String) {
    let word = match v.level {
        Level::Attention => "needs you",
        Level::Failing => "failing",
        Level::Warning => "warning",
        Level::Working => "working",
        Level::Idle => "idle",
    };
    if v.kind == "claude" {
        if let Some(row) = s.sessions.iter().find(|r| r.session_id == v.id) {
            let name = row.cwd.trim_end_matches('/').rsplit('/').next().unwrap_or(&row.cwd);
            return (format!("Claude · {name} · {word}"), row.snippet.clone());
        }
    }
    if let Some(row) = s.panes.iter().find(|r| r.id == v.id) {
        return (format!("{}:{} · {word}", row.session, row.window_name), row.snippet.clone());
    }
    (format!("watchglass · {word}"), String::new())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_shortcuts([tray::SHORTCUT])
                .expect("shortcut")
                .with_handler(|app, _shortcut, event| {
                    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        tray::toggle_panel(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            // A menu bar app: no Dock icon until the board is opened.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let data_dir = app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir().join("watchglass"));
            let _ = std::fs::create_dir_all(&data_dir);
            tmux::set_log_path(data_dir.join("watchglass.log"));
            tmux::log(&format!("start {} pid {}", env!("CARGO_PKG_VERSION"), std::process::id()));

            let verdicts_path = state::default_path().unwrap_or_else(|| data_dir.join("verdicts.json"));
            let first_run = !verdicts_path.exists();
            tmux::log(&format!("verdicts file {}", verdicts_path.display()));

            let bus = Arc::new(BusSlot::default());
            let store = Arc::new(Store::new(verdicts_path));
            let subs = Arc::new(PanelSubs::default());
            let taps = Arc::new(TapManager::new(Arc::clone(&bus), data_dir.clone()));
            let classifier = Classifier::start(Arc::clone(&bus), Some(data_dir.join("verdicts.log")));
            let panes: Arc<Mutex<Vec<PaneInfo>>> = Arc::new(Mutex::new(Vec::new()));
            let handle = app.handle().clone();

            // Output → snippet and classification (debounced inside).
            {
                let taps = Arc::clone(&taps);
                let classifier = Arc::clone(&classifier);
                let panes = Arc::clone(&panes);
                let store = Arc::clone(&store);
                tmux::set_output_hook(Arc::new(move |id: &str| {
                    let pane = panes.lock().expect("panes").iter().find(|p| p.id == id).cloned();
                    if let Some(last) = taps.tail_text(id, 1) {
                        store.set_tail(id, last);
                    }
                    if let (Some(pane), Some(tail)) = (pane, taps.tail_text(id, jev::tail_lines())) {
                        if !tail.trim().is_empty() {
                            classifier.submit(Job::Pane { pane, tail });
                        }
                    }
                }));
            }

            bus.set(
                "service",
                Arc::new(ServiceBus {
                    store: Arc::clone(&store),
                    subs: Arc::clone(&subs),
                    app: handle.clone(),
                    last_notified: Mutex::new(HashMap::new()),
                }),
            );

            let autostart_on = {
                use tauri_plugin_autostart::ManagerExt;
                app.autolaunch().is_enabled().unwrap_or(false)
            };
            tray::build(&handle, autostart_on)?;
            if let Some(panel) = app.get_webview_window(tray::PANEL) {
                let h = handle.clone();
                panel.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        tray::panel_blurred(&h);
                    }
                });
            }

            // Pipes left by a crashed run keep tmux appending forever; close ours.
            if let Ok(list) = tmux::list_panes() {
                let cleaned = taps.cleanup_stale(&list);
                if !cleaned.is_empty() {
                    tmux::log(&format!("closed stale pipes on {}", cleaned.join(", ")));
                }
            }
            taps.start_poller(Arc::clone(&panes));

            // Claude sessions: poll, emit on change, classify on change.
            {
                let bus = Arc::clone(&bus);
                let classifier = Arc::clone(&classifier);
                thread::Builder::new()
                    .name("wg-claude".into())
                    .spawn(move || {
                        let mut last: Option<Vec<ClaudeSession>> = None;
                        loop {
                            let sessions = claude::sessions();
                            if last.as_ref() != Some(&sessions) {
                                for s in &sessions {
                                    classifier.submit(Job::Claude(s.clone()));
                                }
                                bus.claude(sessions.clone());
                                last = Some(sessions);
                            }
                            thread::sleep(Duration::from_millis(2000));
                        }
                    })
                    .expect("claude poller");
            }

            app.manage(AppState { bus, taps, classifier, panes, store, subs });
            if first_run {
                tray::show_panel(&handle);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            subscribe,
            subscribe_summary,
            summary,
            tmux_available,
            tmux_status,
            list_panes,
            attach_pane,
            detach_pane,
            focus_pane,
            send_keys,
            send_line,
            new_window,
            claude_sessions,
            ghostty_appearance,
            jev_status,
            open_main,
            hide_panel,
            verdicts_path
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| match event {
            // Closing the board must not stop the service; only Quit does.
            tauri::RunEvent::ExitRequested { code: None, api, .. } => api.prevent_exit(),
            tauri::RunEvent::Exit => {
                if let Some(state) = app.try_state::<AppState>() {
                    state.taps.detach_all();
                }
            }
            _ => {}
        });
}
