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

    /// ホットキー文字列を登録する。登録に失敗した場合は旧ホットキーを維持する。
    pub fn register(&mut self, hotkey_str: &str) -> Result<(), BoxError> {
        let parsed = Hotkey::parse(hotkey_str)?;
        let target = to_global_hotkey(&parsed)
            .ok_or_else(|| format!("サポートされていないホットキーです: {hotkey_str}"))?;

        // 既存のホットキーを一旦退避・解除
        let old_hotkey = self.current_hotkey.take();
        if let Some(old) = &old_hotkey {
            let _ = self.manager.unregister(*old);
        }

        if let Err(e) = self.manager.register(target) {
            // 新しいキーの登録に失敗した場合、旧キーの復元を試みる
            if let Some(old) = old_hotkey {
                let _ = self.manager.register(old);
                self.current_hotkey = Some(old);
            }
            return Err(e.into());
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    /// 文字列を `Hotkey::parse` で解析し、`to_global_hotkey` で変換する
    fn convert(input: &str) -> Option<HotKey> {
        let parsed = Hotkey::parse(input).expect("app 側で解析できる文字列");
        to_global_hotkey(&parsed)
    }

    /// 変換結果が期待する修飾キーとコードの `HotKey` と一致することを `id()` で確認する
    fn assert_converts_to(input: &str, mods: Modifiers, code: Code) {
        let actual = convert(input).unwrap_or_else(|| panic!("{input} は変換できるはず"));
        let expected = HotKey::new(Some(mods), code);
        assert_eq!(actual.id(), expected.id(), "input: {input}");
    }

    #[test]
    fn converts_ctrl_alt_t() {
        assert_converts_to("Ctrl+Alt+T", Modifiers::CONTROL | Modifiers::ALT, Code::KeyT);
    }

    #[test]
    fn converts_shift_modifier() {
        assert_converts_to("Shift+A", Modifiers::SHIFT, Code::KeyA);
    }

    #[test]
    fn converts_super_modifier_to_meta() {
        assert_converts_to("Super+Z", Modifiers::META, Code::KeyZ);
        // 別名 (Win) でも同じ修飾キーになる
        assert_converts_to("Win+Z", Modifiers::META, Code::KeyZ);
    }

    #[test]
    fn converts_all_modifiers_together() {
        assert_converts_to(
            "Ctrl+Alt+Shift+Super+Q",
            Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT | Modifiers::META,
            Code::KeyQ,
        );
    }

    #[test]
    fn converts_digit_key() {
        assert_converts_to("Ctrl+1", Modifiers::CONTROL, Code::Digit1);
        assert_converts_to("Ctrl+0", Modifiers::CONTROL, Code::Digit0);
    }

    #[test]
    fn converts_function_key() {
        assert_converts_to("Ctrl+F5", Modifiers::CONTROL, Code::F5);
        assert_converts_to("Ctrl+F12", Modifiers::CONTROL, Code::F12);
    }

    #[test]
    fn converts_special_keys() {
        assert_converts_to("Ctrl+Space", Modifiers::CONTROL, Code::Space);
        assert_converts_to("Ctrl+Enter", Modifiers::CONTROL, Code::Enter);
        assert_converts_to("Ctrl+Tab", Modifiers::CONTROL, Code::Tab);
        assert_converts_to("Ctrl+Escape", Modifiers::CONTROL, Code::Escape);
        // 別名 (Esc / Return) は app 側で正規化される
        assert_converts_to("Ctrl+Esc", Modifiers::CONTROL, Code::Escape);
        assert_converts_to("Ctrl+Return", Modifiers::CONTROL, Code::Enter);
    }

    #[test]
    fn returns_none_for_function_keys_above_f12() {
        // app 側は F1〜F24 を受理するが、GUI 側は F1〜F12 のみ対応する
        for key in ["F13", "F20", "F24"] {
            let input = format!("Ctrl+{key}");
            assert!(convert(&input).is_none(), "{input} は変換できないはず");
        }
    }
}
