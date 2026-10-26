//! ウィンドウのライフサイクル判断（ADR-007）。
//!
//! 「×」ボタンで隠すか終了するか、といったトレイの有無で変わる振る舞いを、
//! egui に依存しない純粋関数にまとめる。`app.rs` に条件を散らさず、
//! 設定とトレイの状態の組み合わせをここでテストする。

/// ウィンドウを閉じる操作の結果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseAction {
    /// ウィンドウを隠してトレイに常駐する
    HideToTray,
    /// プロセスを終了する
    Quit,
}

/// 「×」で閉じる要求が来たときの扱いを決める。
///
/// - `use_tray`: 設定で「トレイを使う」になっているか
/// - `tray_active`: トレイアイコンの初期化に成功しているか
/// - `is_quitting`: 終了操作（ヘッダーの「終了」やトレイの「終了」）の途中か
///
/// 隠すのは、トレイを使う設定で、かつトレイが有効で、終了中でないときだけ。
/// それ以外は終了する。トレイが見えない環境で隠れたまま戻れなくなることを防ぐ。
pub fn on_close_requested(use_tray: bool, tray_active: bool, is_quitting: bool) -> CloseAction {
    if use_tray && tray_active && !is_quitting {
        CloseAction::HideToTray
    } else {
        CloseAction::Quit
    }
}

/// 「トレイに隠す」ボタンを出すか。
pub fn can_hide_to_tray(use_tray: bool, tray_active: bool) -> bool {
    use_tray && tray_active
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hides_only_when_enabled_active_and_not_quitting() {
        assert_eq!(on_close_requested(true, true, false), CloseAction::HideToTray);
    }

    #[test]
    fn quits_when_tray_is_disabled_by_setting() {
        assert_eq!(on_close_requested(false, true, false), CloseAction::Quit);
    }

    #[test]
    fn quits_when_tray_failed_to_initialize() {
        assert_eq!(on_close_requested(true, false, false), CloseAction::Quit);
    }

    #[test]
    fn quits_when_neither_setting_nor_tray() {
        assert_eq!(on_close_requested(false, false, false), CloseAction::Quit);
    }

    #[test]
    fn quits_while_quitting_even_if_tray_is_usable() {
        assert_eq!(on_close_requested(true, true, true), CloseAction::Quit);
    }

    #[test]
    fn close_decision_covers_every_combination() {
        // 8 通りすべてを確認する。隠すのは (true, true, false) のみ
        for use_tray in [false, true] {
            for tray_active in [false, true] {
                for is_quitting in [false, true] {
                    let expected = if use_tray && tray_active && !is_quitting {
                        CloseAction::HideToTray
                    } else {
                        CloseAction::Quit
                    };
                    assert_eq!(
                        on_close_requested(use_tray, tray_active, is_quitting),
                        expected,
                        "use_tray={use_tray}, tray_active={tray_active}, is_quitting={is_quitting}"
                    );
                }
            }
        }
    }

    #[test]
    fn hide_button_requires_setting_and_active_tray() {
        assert!(can_hide_to_tray(true, true));
        assert!(!can_hide_to_tray(false, true));
        assert!(!can_hide_to_tray(true, false));
        assert!(!can_hide_to_tray(false, false));
    }

    #[test]
    fn hide_button_and_close_are_consistent() {
        // ボタンが出るときは、「×」でも隠れる（終了中を除く）。ずれを防ぐ
        for use_tray in [false, true] {
            for tray_active in [false, true] {
                assert_eq!(
                    can_hide_to_tray(use_tray, tray_active),
                    on_close_requested(use_tray, tray_active, false) == CloseAction::HideToTray
                );
            }
        }
    }
}
