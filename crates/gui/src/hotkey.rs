use eframe::egui;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use tray_translate_app::{Hotkey, Modifier};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// OS のグローバルホットキー管理
pub struct HotkeyManager {
    manager: GlobalHotKeyManager,
    current_hotkey: Option<HotKey>,
}

impl HotkeyManager {
    /// グローバルホットキーマネージャを初期化し、初期ホットキーを登録する
    pub fn new(ctx: egui::Context, initial_hotkey_str: &str) -> Result<Self, BoxError> {
        let manager = GlobalHotKeyManager::new()?;

        // ホットキー押下時に非表示ウィンドウでも起床できるように repaint コールバックを登録
        GlobalHotKeyEvent::set_event_handler(Some(move |_| {
            ctx.request_repaint();
        }));

        let mut instance = Self {
            manager,
            current_hotkey: None,
        };

        instance.register(initial_hotkey_str)?;
        Ok(instance)
    }

    /// ホットキー文字列を登録する。登録済みのものがあれば解除してから再登録する。
    pub fn register(&mut self, hotkey_str: &str) -> Result<(), BoxError> {
        // 既存のホットキーを解除
        if let Some(old) = self.current_hotkey.take() {
            let _ = self.manager.unregister(old);
        }

        let parsed = Hotkey::parse(hotkey_str)?;
        let target = to_global_hotkey(&parsed)
            .ok_or_else(|| format!("サポートされていないホットキーです: {hotkey_str}"))?;

        self.manager.register(target)?;
        self.current_hotkey = Some(target);
        Ok(())
    }

    /// ホットキーが押下されたかどうかをポーリングする
    pub fn poll_pressed(&self) -> bool {
        let mut pressed = false;
        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.state == HotKeyState::Pressed {
                if let Some(current) = &self.current_hotkey {
                    if event.id == current.id() {
                        pressed = true;
                    }
                }
            }
        }
        pressed
    }
}

/// `crates/app` の `Hotkey` を `global_hotkey::hotkey::HotKey` へ変換する
fn to_global_hotkey(hotkey: &Hotkey) -> Option<HotKey> {
    let mut mods = Modifiers::empty();
    for m in hotkey.modifiers() {
        match m {
            Modifier::Ctrl => mods |= Modifiers::CONTROL,
            Modifier::Alt => mods |= Modifiers::ALT,
            Modifier::Shift => mods |= Modifiers::SHIFT,
            Modifier::Super => mods |= Modifiers::META,
        }
    }

    let code = match hotkey.key() {
        "A" => Code::KeyA,
        "B" => Code::KeyB,
        "C" => Code::KeyC,
        "D" => Code::KeyD,
        "E" => Code::KeyE,
        "F" => Code::KeyF,
        "G" => Code::KeyG,
        "H" => Code::KeyH,
        "I" => Code::KeyI,
        "J" => Code::KeyJ,
        "K" => Code::KeyK,
        "L" => Code::KeyL,
        "M" => Code::KeyM,
        "N" => Code::KeyN,
        "O" => Code::KeyO,
        "P" => Code::KeyP,
        "Q" => Code::KeyQ,
        "R" => Code::KeyR,
        "S" => Code::KeyS,
        "T" => Code::KeyT,
        "U" => Code::KeyU,
        "V" => Code::KeyV,
        "W" => Code::KeyW,
        "X" => Code::KeyX,
        "Y" => Code::KeyY,
        "Z" => Code::KeyZ,
        "0" => Code::Digit0,
        "1" => Code::Digit1,
        "2" => Code::Digit2,
        "3" => Code::Digit3,
        "4" => Code::Digit4,
        "5" => Code::Digit5,
        "6" => Code::Digit6,
        "7" => Code::Digit7,
        "8" => Code::Digit8,
        "9" => Code::Digit9,
        "F1" => Code::F1,
        "F2" => Code::F2,
        "F3" => Code::F3,
        "F4" => Code::F4,
        "F5" => Code::F5,
        "F6" => Code::F6,
        "F7" => Code::F7,
        "F8" => Code::F8,
        "F9" => Code::F9,
        "F10" => Code::F10,
        "F11" => Code::F11,
        "F12" => Code::F12,
        "Space" => Code::Space,
        "Enter" => Code::Enter,
        "Tab" => Code::Tab,
        "Escape" => Code::Escape,
        _ => return None,
    };

    Some(HotKey::new(Some(mods), code))
}
