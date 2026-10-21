use eframe::egui;
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

        // トレイイベント発生時にバックグラウンドのウィンドウを起床させる
        let ctx_tray = ctx.clone();
        TrayIconEvent::set_event_handler(Some(move |_| {
            ctx_tray.request_repaint();
        }));

        let ctx_menu = ctx;
        MenuEvent::set_event_handler(Some(move |_| {
            ctx_menu.request_repaint();
        }));

        Ok(Self {
            _tray_icon: tray_icon,
        })
    }

    /// トレイ関連のイベント（アイコンクリック、メニュー選択）をポーリングする
    pub fn poll_events(&self) -> Option<TrayAction> {
        // メニューのクリックイベント
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id.as_ref() == "show" {
                return Some(TrayAction::Show);
            }
            if event.id.as_ref() == "quit" {
                return Some(TrayAction::Quit);
            }
        }

        // トレイアイコン本体のクリックイベント（左クリックでウィンドウ表示）
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            match event {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
                | TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                } => {
                    return Some(TrayAction::Show);
                }
                _ => {}
            }
        }

        None
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
            let is_corner = (x < 3 || x >= WIDTH - 3) && (y < 3 || y >= HEIGHT - 3);
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
