//! The menus: File, Recording, View and Help, with keyboard shortcuts.
//!
//! On macOS they are real menus in the menu bar at the top of the screen. On Windows and Linux
//! the same menus are drawn at the top of the window, and the shortcuts use Ctrl instead of ⌘.

use egui::{Key, KeyboardShortcut, Modifiers};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    NewMeeting,
    CopyText,
    SaveToDownloads,
    Delete,
    RecordStop,
    PauseRecording,
    PlayPause,
    Summary,
    Transcript,
    Notes,
    Ask,
    Search,
    Guide,
    Settings,
    Quit,
}

/// What the menus need to know to show the right words and grey out what can't be used.
#[derive(Clone, PartialEq, Default, Debug)]
pub struct State {
    pub has_meeting: bool,
    pub recording: bool,
    pub paused: bool,
    pub has_audio: bool,
    pub playing: bool,
}

const ALL: &[Action] = &[
    Action::NewMeeting,
    Action::CopyText,
    Action::SaveToDownloads,
    Action::Delete,
    Action::RecordStop,
    Action::PauseRecording,
    Action::PlayPause,
    Action::Summary,
    Action::Transcript,
    Action::Notes,
    Action::Ask,
    Action::Search,
    Action::Guide,
    Action::Settings,
    Action::Quit,
];

impl Action {
    fn id(self) -> &'static str {
        match self {
            Action::NewMeeting => "new",
            Action::CopyText => "copy",
            Action::SaveToDownloads => "save",
            Action::Delete => "delete",
            Action::RecordStop => "record",
            Action::PauseRecording => "pause",
            Action::PlayPause => "play",
            Action::Summary => "summary",
            Action::Transcript => "transcript",
            Action::Notes => "notes",
            Action::Ask => "ask",
            Action::Search => "search",
            Action::Guide => "guide",
            Action::Settings => "settings",
            Action::Quit => "quit",
        }
    }

    pub fn from_id(id: &str) -> Option<Action> {
        ALL.iter().copied().find(|a| a.id() == id)
    }

    /// The words shown in the menu, which change with what's happening.
    pub fn label(self, st: &State) -> &'static str {
        match self {
            Action::NewMeeting => "New Meeting",
            Action::CopyText => "Copy Meeting as Text",
            Action::SaveToDownloads => "Save to Downloads",
            Action::Delete => "Delete Meeting…",
            Action::RecordStop if st.recording => "Stop Recording",
            Action::RecordStop => "Record",
            Action::PauseRecording if st.paused => "Resume Recording",
            Action::PauseRecording => "Pause Recording",
            Action::PlayPause if st.playing => "Pause Playback",
            Action::PlayPause => "Play Recording",
            Action::Summary => "Summary",
            Action::Transcript => "Transcript",
            Action::Notes => "My Notes",
            Action::Ask => "Ask",
            Action::Search => "Search Meetings",
            Action::Guide => "Silvery User Guide",
            Action::Settings => "Settings…",
            Action::Quit => "Quit Silvery",
        }
    }

    pub fn enabled(self, st: &State) -> bool {
        match self {
            Action::CopyText | Action::SaveToDownloads | Action::Delete => st.has_meeting,
            Action::RecordStop => st.has_meeting || st.recording,
            Action::PauseRecording => st.recording,
            Action::PlayPause => st.has_audio && !st.recording,
            Action::Summary | Action::Transcript | Action::Notes | Action::Ask => st.has_meeting,
            _ => true,
        }
    }

    /// ⌘ on macOS, Ctrl elsewhere.
    pub fn shortcut(self) -> Option<KeyboardShortcut> {
        let cmd = Modifiers::COMMAND;
        let cmd_shift = Modifiers::COMMAND | Modifiers::SHIFT;
        Some(match self {
            Action::NewMeeting => KeyboardShortcut::new(cmd, Key::N),
            Action::CopyText => KeyboardShortcut::new(cmd_shift, Key::C),
            Action::SaveToDownloads => KeyboardShortcut::new(cmd, Key::S),
            Action::Delete => KeyboardShortcut::new(cmd, Key::Backspace),
            Action::RecordStop => KeyboardShortcut::new(cmd, Key::R),
            Action::PauseRecording => KeyboardShortcut::new(cmd_shift, Key::R),
            Action::PlayPause => KeyboardShortcut::new(cmd_shift, Key::P),
            Action::Summary => KeyboardShortcut::new(cmd, Key::Num1),
            Action::Transcript => KeyboardShortcut::new(cmd, Key::Num2),
            Action::Notes => KeyboardShortcut::new(cmd, Key::Num3),
            Action::Ask => KeyboardShortcut::new(cmd, Key::Num4),
            Action::Search => KeyboardShortcut::new(cmd, Key::F),
            Action::Guide => KeyboardShortcut::new(cmd_shift, Key::Slash),
            Action::Settings => KeyboardShortcut::new(cmd, Key::Comma),
            Action::Quit => KeyboardShortcut::new(cmd, Key::Q),
        })
    }
}

/// The menus and what's in them, in order. `None` is a separator line.
const MENUS: &[(&str, &[Option<Action>])] = &[
    (
        "File",
        &[
            Some(Action::NewMeeting),
            None,
            Some(Action::CopyText),
            Some(Action::SaveToDownloads),
            None,
            Some(Action::Delete),
        ],
    ),
    (
        "Recording",
        &[Some(Action::RecordStop), Some(Action::PauseRecording), None, Some(Action::PlayPause)],
    ),
    (
        "View",
        &[
            Some(Action::Summary),
            Some(Action::Transcript),
            Some(Action::Notes),
            Some(Action::Ask),
            None,
            Some(Action::Search),
        ],
    ),
    ("Help", &[Some(Action::Guide)]),
];

// ---------------------------------------------------------------- Windows / Linux

/// Shortcuts typed in the window (used where the menus aren't native).
#[cfg_attr(target_os = "macos", allow(dead_code))]
pub fn shortcuts(ctx: &egui::Context, st: &State) -> Vec<Action> {
    let mut out = Vec::new();
    ctx.input_mut(|i| {
        for a in ALL {
            if let Some(sc) = a.shortcut() {
                if i.consume_shortcut(&sc) && a.enabled(st) {
                    out.push(*a);
                }
            }
        }
    });
    out
}

/// The menu bar drawn at the top of the window.
#[cfg_attr(target_os = "macos", allow(dead_code))]
pub fn window_menu_bar(ui: &mut egui::Ui, st: &State) -> Option<Action> {
    let mut chosen = None;
    let ctx = ui.ctx().clone();
    egui::MenuBar::new().ui(ui, |ui| {
        for (title, items) in MENUS {
            ui.menu_button(*title, |ui| {
                ui.set_min_width(220.0);
                let extra: &[Option<Action>] = match *title {
                    "File" => &[None, Some(Action::Settings), None, Some(Action::Quit)],
                    _ => &[],
                };
                for item in items.iter().chain(extra.iter()) {
                    match item {
                        None => {
                            ui.separator();
                        }
                        Some(a) => {
                            let mut b = egui::Button::new(a.label(st));
                            if let Some(sc) = a.shortcut() {
                                b = b.shortcut_text(ctx.format_shortcut(&sc));
                            }
                            if ui.add_enabled(a.enabled(st), b).clicked() {
                                chosen = Some(*a);
                                ui.close();
                            }
                        }
                    }
                }
            });
        }
    });
    chosen
}

// ---------------------------------------------------------------- macOS

#[cfg(target_os = "macos")]
pub use native::NativeMenu;

#[cfg(target_os = "macos")]
mod native {
    use super::{Action, State, ALL, MENUS};
    use muda::accelerator::{Accelerator, Code, Modifiers as M};
    use muda::{AboutMetadata, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
    use std::collections::HashMap;
    use std::sync::mpsc::Sender;

    fn accelerator(a: Action) -> Option<Accelerator> {
        let cmd = M::SUPER;
        let cmd_shift = M::SUPER | M::SHIFT;
        let (m, code) = match a {
            Action::NewMeeting => (cmd, Code::KeyN),
            Action::CopyText => (cmd_shift, Code::KeyC),
            Action::SaveToDownloads => (cmd, Code::KeyS),
            Action::Delete => (cmd, Code::Backspace),
            Action::RecordStop => (cmd, Code::KeyR),
            Action::PauseRecording => (cmd_shift, Code::KeyR),
            Action::PlayPause => (cmd_shift, Code::KeyP),
            Action::Summary => (cmd, Code::Digit1),
            Action::Transcript => (cmd, Code::Digit2),
            Action::Notes => (cmd, Code::Digit3),
            Action::Ask => (cmd, Code::Digit4),
            Action::Search => (cmd, Code::KeyF),
            Action::Guide => (cmd_shift, Code::Slash),
            Action::Settings => (cmd, Code::Comma),
            Action::Quit => (cmd, Code::KeyQ),
        };
        Some(Accelerator::new(Some(m), code))
    }

    pub struct NativeMenu {
        _menu: Menu,
        items: HashMap<&'static str, (Action, MenuItem)>,
        last: Option<State>,
    }

    impl NativeMenu {
        /// Put Silvery's menus in the macOS menu bar. Chosen items are sent to `tx`.
        pub fn install(tx: Sender<Action>, ctx: egui::Context, icon: Option<muda::Icon>) -> Option<NativeMenu> {
            let st = State::default();
            let mut items = HashMap::new();
            let mut make = |a: Action| {
                let it = MenuItem::with_id(a.id(), a.label(&st), a.enabled(&st), accelerator(a));
                items.insert(a.id(), (a, it.clone()));
                it
            };

            let menu = Menu::new();

            // The first menu is the app menu ("Silvery").
            let about = PredefinedMenuItem::about(
                Some("About Silvery"),
                Some(AboutMetadata {
                    name: Some("Silvery".into()),
                    version: Some(env!("CARGO_PKG_VERSION").into()),
                    comments: Some("Private meeting notes with a local AI.".into()),
                    license: Some("MIT".into()),
                    icon,
                    ..Default::default()
                }),
            );
            let app = Submenu::new("Silvery", true);
            app.append_items(&[
                &about,
                &PredefinedMenuItem::separator(),
                &make(Action::Settings),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::hide(None),
                &PredefinedMenuItem::hide_others(None),
                &PredefinedMenuItem::show_all(None),
                &PredefinedMenuItem::separator(),
                &make(Action::Quit),
            ])
            .ok()?;
            menu.append(&app).ok()?;

            for (title, entries) in MENUS {
                let sub = Submenu::new(*title, true);
                for e in *entries {
                    match e {
                        None => sub.append(&PredefinedMenuItem::separator()).ok()?,
                        Some(a) => sub.append(&make(*a)).ok()?,
                    }
                }
                menu.append(&sub).ok()?;
            }

            menu.init_for_nsapp();
            // SILVERY_MENU_LOG=1 lists the installed menus (for checks).
            if std::env::var_os("SILVERY_MENU_LOG").is_some() {
                for it in menu.items() {
                    if let Some(sub) = it.as_submenu() {
                        let names: Vec<String> = sub
                            .items()
                            .iter()
                            .filter_map(|i| i.as_menuitem().map(|m| m.text()))
                            .collect();
                        eprintln!("menu {}: {}", sub.text(), names.join(", "));
                    }
                }
            }
            MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
                if let Some(a) = Action::from_id(e.id.0.as_str()) {
                    let _ = tx.send(a);
                    ctx.request_repaint();
                }
            }));
            let _ = ALL;
            Some(NativeMenu { _menu: menu, items, last: None })
        }

        /// Update the words and greyed-out items when something changed.
        pub fn update(&mut self, st: &State) {
            if self.last.as_ref() == Some(st) {
                return;
            }
            for (a, item) in self.items.values() {
                item.set_text(a.label(st));
                item.set_enabled(a.enabled(st));
            }
            self.last = Some(st.clone());
        }
    }
}
