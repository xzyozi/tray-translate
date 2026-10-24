use eframe::egui;
use std::sync::mpsc::{channel, Receiver};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// トレイからの要求操作
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    /// 翻訳ウィンドウを表示・最前面化
    Show,
    /// アプリケーション終了
    Quit,
}

/// トレイアイコンの管理。Drop されるとトレイアイコンが削除されるため保持する。
pub struct TrayManager {
    _tray_icon: TrayIcon,
    action_rx: Receiver<TrayAction>,
}

impl TrayManager {
    /// トレイアイコンとメニューを初期化する。
    /// 非表示状態でもトレイ操作で即座に UI ループが起床するよう、egui::Context の repaint を登録する。
    pub fn new(ctx: egui::Context) -> Result<Self, BoxError> {
        let icon = create_default_icon()?;

        let menu = Menu::new();
        let show_item = MenuItem::with_id("show", "翻訳ウィンドウを表示", true, None);
        let separator = PredefinedMenuItem::separator();
        let quit_item = MenuItem::with_id("quit", "終了", true, None);

        menu.append_items(&[&show_item, &separator, &quit_item])?;

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("tray-translate")
            .with_icon(icon)
            .build()?;

        let (action_tx, action_rx) = channel();

        // トレイイベント発生時にキューへ格納し、バックグラウンドのウィンドウを起床させる
        // ※ tray-icon および muda では set_event_handler(Some(...)) を設定すると
        //   TrayIconEvent::receiver() や MenuEvent::receiver() への送信が行われなくなるため、
        //   自前のチャネルに送信して管理する。
        let tx_tray = action_tx.clone();
        let ctx_tray = ctx.clone();
        TrayIconEvent::set_event_handler(Some(move |event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }
            | TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => {
                let _ = tx_tray.send(TrayAction::Show);
                ctx_tray.request_repaint();
            }
            _ => {}
        }));

        let tx_menu = action_tx;
        let ctx_menu = ctx;
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if event.id.as_ref() == "show" {
                let _ = tx_menu.send(TrayAction::Show);
                ctx_menu.request_repaint();
            } else if event.id.as_ref() == "quit" {
                let _ = tx_menu.send(TrayAction::Quit);
                ctx_menu.request_repaint();
            }
        }));

        Ok(Self {
            _tray_icon: tray_icon,
            action_rx,
        })
    }

    /// トレイ関連のイベント（アイコンクリック、メニュー選択）をポーリングする
    pub fn poll_events(&self) -> Option<TrayAction> {
        let mut last_action = None;
        while let Ok(action) = self.action_rx.try_recv() {
            if action == TrayAction::Quit {
                return Some(TrayAction::Quit);
            }
            last_action = Some(action);
        }
        last_action
    }
}

/// 32x32 のシンプルな青地に「T」マークの RGBA アイコンを生成する
fn create_default_icon() -> Result<Icon, BoxError> {
    const WIDTH: u32 = 32;
    const HEIGHT: u32 = 32;
    let mut rgba = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);

    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            // 外枠を少し丸める（角の3ピクセルは透明）
            let is_corner = !(3..WIDTH - 3).contains(&x) && !(3..HEIGHT - 3).contains(&y);
            if is_corner {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }

            // 中央の「T」の文字領域（白: 255, 255, 255）
            // 水平バー: y in 7..11, x in 7..25
            let is_t_bar = (7..11).contains(&y) && (7..25).contains(&x);
            // 垂直ステム: y in 11..25, x in 14..18
            let is_t_stem = (11..25).contains(&y) && (14..18).contains(&x);

            if is_t_bar || is_t_stem {
                rgba.extend_from_slice(&[255, 255, 255, 255]);
            } else {
                // 背景: 青 (#2B6CB0: R=43, G=108, B=176)
                rgba.extend_from_slice(&[43, 108, 176, 255]);
            }
        }
    }

    Ok(Icon::from_rgba(rgba, WIDTH, HEIGHT)?)
}
