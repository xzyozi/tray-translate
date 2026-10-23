use crate::clipboard::ArboardClipboard;
use crate::fonts::install_japanese_font;
use crate::hotkey::HotkeyManager;
use crate::models_view::ModelsView;
use crate::theme;
use crate::tray::{TrayAction, TrayManager};
use eframe::egui;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};
use tray_translate_app::{
    get_settings_path, models::required_packages, Command, Controller, Event, Settings, SystemClock,
};
use tray_translate_core::TranslationRouter;

/// クリップボード確認とアイドル解放の確認の間隔
const TICK_INTERVAL: Duration = Duration::from_secs(1);

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// キューに溜まった tick を 1 回分に集約した結果
struct TickBatch {
    /// クリップボード確認を行うか
    clipboard: bool,
    /// アイドル解放の確認を行うか
    idle: bool,
    /// tick の後ろで最初に見つかった、tick 以外の操作（処理順を保つため、tick の後に実行する）
    next: Option<Command>,
}

/// `first`（tick）に続く tick をキューから取り出して集約する。
/// tick 以外の操作に当たったら、そこで止めて `next` に返す。
fn coalesce_ticks(first: &Command, rx: &Receiver<Command>) -> TickBatch {
    let mut batch = TickBatch {
        clipboard: matches!(first, Command::ClipboardTick),
        idle: matches!(first, Command::IdleTick),
        next: None,
    };

    while let Ok(command) = rx.try_recv() {
        match command {
            Command::ClipboardTick => batch.clipboard = true,
            Command::IdleTick => batch.idle = true,
            other => {
                batch.next = Some(other);
                break;
            }
        }
    }
    batch
}

/// 画面タブ
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Translate,
    Models,
    Settings,
}

/// 翻訳ウィンドウ。制御は別スレッドの `Controller` に任せ、ここでは表示と入力の受け渡しだけを行う。
pub struct GuiApp {
    cmd_tx: Sender<Command>,
    event_rx: Receiver<Event>,
    tray: Option<TrayManager>,
    hotkey: Option<HotkeyManager>,
    /// ウィンドウが表示されているか
    is_window_visible: bool,
    /// 現在表示中のタブ
    active_tab: ActiveTab,
    /// モデル管理ビュー
    models_view: ModelsView,
    /// 導入済みのモデル（ペアキー）
    installed: Vec<String>,
    input: String,
    output: String,
    status: String,
    source_lang: String,
    target_lang: String,
    hotkey_str: String,
    idle_unload_secs: u64,
    watch_clipboard: bool,
    copy_result: bool,
    min_chars: usize,
    max_chars: usize,
    /// 翻訳処理中かどうかのフラグ
    is_translating: bool,
    /// アプリ終了シーケンス中かどうかのフラグ（CancelClose を抑制する）
    is_quitting: bool,
    /// 自動コピーする翻訳結果（次のフレームの先頭でクリップボードへ書く）
    to_copy: Option<String>,
    last_tick: Instant,
}

impl GuiApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Result<Self, BoxError> {
        theme::apply(&cc.egui_ctx);
        let font = install_japanese_font(&cc.egui_ctx);

        let settings_path = get_settings_path()?;
        let settings = Settings::load(&settings_path)?;
        let router = TranslationRouter::new()?;
        let installed = router.available_pairs();

        let source_lang = settings.source_lang.clone();
        let target_lang = settings.target_lang.clone();
        let hotkey_str = settings.hotkey.clone();
        let idle_unload_secs = settings.idle_unload_secs;
        let watch_clipboard = settings.watch_clipboard;
        let copy_result = settings.copy_result;
        let min_chars = settings.min_chars;
        let max_chars = settings.max_chars;

        let mut controller = Controller::new(
            settings,
            Some(settings_path),
            router,
            SystemClock::new(),
            Box::new(ArboardClipboard::default()),
        )?;

        let (cmd_tx, cmd_rx) = channel::<Command>();
        let (event_tx, event_rx) = channel::<Event>();

        // 翻訳などの重い処理をワーカースレッドで非同期実行し、UI の応答性を確保する
        let egui_ctx = cc.egui_ctx.clone();
        std::thread::Builder::new()
            .name("translation-worker".to_string())
            .spawn(move || {
                while let Ok(mut cmd) = cmd_rx.recv() {
                    // tick コマンドがキューに溜まっていたら 1 回分に集約して無駄な連続処理を間引く
                    if matches!(cmd, Command::ClipboardTick | Command::IdleTick) {
                        let batch = coalesce_ticks(&cmd, &cmd_rx);

                        let mut ticks = Vec::new();
                        if batch.clipboard {
                            ticks.push(Command::ClipboardTick);
                        }
                        if batch.idle {
                            ticks.push(Command::IdleTick);
                        }
                        for tick in ticks {
                            for ev in controller.handle(tick) {
                                let _ = event_tx.send(ev);
                            }
                        }
                        egui_ctx.request_repaint();

                        // tick の後ろに別の操作が続いていたら、それを通常どおり処理する
                        match batch.next {
                            Some(next) => cmd = next,
                            None => continue,
                        }
                    }

                    let events = controller.handle(cmd);
                    let mut has_events = false;
                    for event in events {
                        has_events = true;
                        if event_tx.send(event).is_err() {
                            return;
                        }
                    }
                    if has_events {
                        egui_ctx.request_repaint();
                    }
                }
            })?;

        // トレイアイコンの初期化（失敗しても通常ウィンドウとして動作）
        let tray = match TrayManager::new(cc.egui_ctx.clone()) {
            Ok(t) => Some(t),
            Err(e) => {
                eprintln!("トレイアイコンの初期化をスキップしました: {e}");
                None
            }
        };

        // バックグラウンドで定期的にクリップボード確認・アイドル解放の tick を送信
        let tick_tx = cmd_tx.clone();
        let tick_ctx = cc.egui_ctx.clone();
        std::thread::Builder::new()
            .name("gui-ticker".to_string())
            .spawn(move || loop {
                std::thread::sleep(TICK_INTERVAL);
                if tick_tx.send(Command::ClipboardTick).is_err() {
                    break;
                }
                if tick_tx.send(Command::IdleTick).is_err() {
                    break;
                }
                tick_ctx.request_repaint();
            })?;

        // グローバルホットキーの初期化（失敗しても通常ウィンドウとして動作）
        let hotkey = match HotkeyManager::new(cc.egui_ctx.clone(), &hotkey_str) {
            Ok(hk) => Some(hk),
            Err(e) => {
                eprintln!("グローバルホットキーの登録をスキップしました: {e}");
                None
            }
        };

        let status = if font.is_none() {
            "日本語フォントが見つかりません（日本語が四角で表示される場合があります）".to_string()
        } else {
            "準備ができました".to_string()
        };

        Ok(Self {
            cmd_tx,
            event_rx,
            tray,
            hotkey,
            is_window_visible: true,
            active_tab: ActiveTab::Translate,
            models_view: ModelsView::new(),
            installed,
            input: String::new(),
            output: String::new(),
            status,
            source_lang,
            target_lang,
            hotkey_str,
            idle_unload_secs,
            watch_clipboard,
            copy_result,
            min_chars,
            max_chars,
            is_translating: false,
            is_quitting: false,
            to_copy: None,
            last_tick: Instant::now(),
        })
    }

    /// トレイ、ホットキー、ウィンドウの閉じる要求を処理する
    fn handle_os_events(&mut self, ctx: &egui::Context) {
        // 1. グローバルホットキー押下
        if let Some(hotkey) = &self.hotkey {
            if hotkey.poll_pressed() {
                self.is_window_visible = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
        }

        // 2. トレイからの操作
        if let Some(tray) = &self.tray {
            if let Some(action) = tray.poll_events() {
                match action {
                    TrayAction::Show => {
                        self.is_window_visible = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    }
                    TrayAction::Quit => {
                        self.is_quitting = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
        }

        // 3. ウィンドウの「×」ボタン押下をインターセプト
        if ctx.input(|i| i.viewport().close_requested()) && self.tray.is_some() && !self.is_quitting
        {
            // トレイアイコンが存在し終了フラグが立っていない場合は、終了せずにウィンドウを隠す
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            self.is_window_visible = false;
        }
    }

    /// コントローラからの通知を画面の状態へ反映する
    fn process_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                Event::Translated { source, output } => {
                    self.is_translating = false;
                    let mut status = "翻訳しました".to_string();
                    if self.input.trim() != source.trim() {
                        self.input = source;
                        status = "クリップボードの内容を翻訳しました".to_string();
                    }
                    if self.copy_result {
                        self.to_copy = Some(output.clone());
                        status.push_str("（結果をクリップボードにコピーしました）");
                    }
                    self.status = status;
                    self.output = output;
                }
                Event::TextIgnored => {
                    self.is_translating = false;
                    self.status = "文字数が範囲外のため翻訳しませんでした".to_string();
                }
                Event::ModelsUnloaded => {
                    self.status = "しばらく使われなかったため、モデルを解放しました".to_string();
                }
                Event::SettingsUpdated { hotkey_changed } => {
                    if hotkey_changed {
                        if let Some(hotkey) = &mut self.hotkey {
                            if let Err(e) = hotkey.register(&self.hotkey_str) {
                                self.status = format!("ホットキー再登録失敗（旧キーを維持）: {e}");
                            } else {
                                self.status = "設定とホットキーを更新しました".to_string();
                            }
                        }
                    } else {
                        self.status = "設定を保存しました".to_string();
                    }
                }
                Event::DownloadStarted { pair } => {
                    // コントローラが受理した時点で、ダウンロード用のスレッドを起動する
                    self.models_view.on_download_started(&pair, self.cmd_tx.clone(), ctx.clone());
                    self.status = format!("ダウンロードを開始しました: {pair}");
                }
                Event::DownloadProgress {
                    pair,
                    downloaded_bytes,
                    total_bytes,
                } => {
                    self.models_view.download_progress = Some((downloaded_bytes, total_bytes));
                    if let Some(total) = total_bytes {
                        let pct =
                            (downloaded_bytes as f64 / total as f64 * 100.0).clamp(0.0, 100.0);
                        self.status = format!("ダウンロード中: {pair} ({pct:.0}%)");
                    } else {
                        let mb = downloaded_bytes as f64 / 1_048_576.0;
                        self.status = format!("ダウンロード中: {pair} ({mb:.1} MB)");
                    }
                }
                Event::DownloadSucceeded { pair } => {
                    self.models_view.clear_download();
                    self.status = format!("ダウンロードが完了しました: {pair}");
                }
                Event::DownloadFailed { pair, message } => {
                    self.models_view.clear_download();
                    self.status = format!("ダウンロードに失敗しました: {pair} ({message})");
                }
                Event::ModelsRefreshed { available_pairs } => {
                    self.installed = available_pairs;
                }
                Event::Error { message } => {
                    self.is_translating = false;
                    // ダウンロード開始の要求が拒否された場合は、待機を取り消す
                    self.models_view.on_download_rejected();
                    self.status = format!("エラー: {message}");
                }
            }
        }
    }

    /// 画面の設定値をコントローラへ反映・保存する
    fn update_settings(&mut self) {
        let settings = Settings {
            source_lang: self.source_lang.trim().to_ascii_lowercase(),
            target_lang: self.target_lang.trim().to_ascii_lowercase(),
            hotkey: self.hotkey_str.trim().to_string(),
            idle_unload_secs: self.idle_unload_secs,
            watch_clipboard: self.watch_clipboard,
            copy_result: self.copy_result,
            min_chars: self.min_chars,
            max_chars: self.max_chars,
        };

        let _ = self.cmd_tx.send(Command::UpdateSettings(settings));
    }

    /// 一定間隔で、クリップボードの確認とアイドル解放の確認を行う
    fn run_ticks(&mut self, ctx: &egui::Context) {
        if self.last_tick.elapsed() >= TICK_INTERVAL {
            self.last_tick = Instant::now();

            let _ = self.cmd_tx.send(Command::ClipboardTick);
            let _ = self.cmd_tx.send(Command::IdleTick);
        }
        ctx.request_repaint_after(TICK_INTERVAL);
    }

    /// 自動コピーの対象があれば、クリップボードへ書く
    fn copy_pending_result(&mut self, ctx: &egui::Context) {
        if let Some(text) = self.to_copy.take() {
            let _ = self
                .cmd_tx
                .send(Command::NoteClipboardWritten(text.clone()));
            ctx.copy_text(text);
        }
    }

    fn missing_packages(&self) -> Vec<String> {
        let from = self.source_lang.trim().to_ascii_lowercase();
        let to = self.target_lang.trim().to_ascii_lowercase();
        required_packages(&self.installed, &from, &to)
    }

    /// 翻訳メイン画面
    fn render_translate_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("翻訳元");
            let src_res =
                ui.add(egui::TextEdit::singleline(&mut self.source_lang).desired_width(40.0));
            ui.label("→ 翻訳先");
            let dst_res =
                ui.add(egui::TextEdit::singleline(&mut self.target_lang).desired_width(40.0));

            // 言語入力が変更されたらコントローラの設定へ即時反映する
            if (src_res.changed()
                || dst_res.changed()
                || src_res.lost_focus()
                || dst_res.lost_focus())
                && !self.source_lang.trim().is_empty()
                && !self.target_lang.trim().is_empty()
            {
                self.update_settings();
            }

            let missing = self.missing_packages();
            if !missing.is_empty() {
                ui.separator();
                ui.colored_label(theme::WARNING, format!("未導入: {}", missing.join(", ")));
                if ui.button("モデル管理で取得").clicked() {
                    self.active_tab = ActiveTab::Models;
                }
            }
        });

        ui.add_space(4.0);
        ui.label("翻訳するテキスト");
        ui.add(
            egui::TextEdit::multiline(&mut self.input)
                .desired_rows(5)
                .desired_width(f32::INFINITY),
        );

        ui.horizontal(|ui| {
            let btn_text = if self.is_translating {
                "翻訳中…"
            } else {
                "翻訳"
            };
            let btn = ui.add_enabled(!self.is_translating, egui::Button::new(btn_text));
            if btn.clicked() {
                self.update_settings();
                self.status = "翻訳中…（初回はモデルの読み込みに時間がかかります）".to_string();
                self.is_translating = true;
                let _ = self.cmd_tx.send(Command::TranslateText(self.input.clone()));
            }
            if ui.button("結果をコピー").clicked() {
                let _ = self
                    .cmd_tx
                    .send(Command::NoteClipboardWritten(self.output.clone()));
                ui.ctx().copy_text(self.output.clone());
                self.status = "結果をクリップボードにコピーしました".to_string();
            }
            if ui.button("クリア").clicked() {
                self.input.clear();
                self.output.clear();
            }
        });

        ui.add_space(4.0);
        ui.label("翻訳結果");
        ui.add(
            egui::TextEdit::multiline(&mut self.output.as_str())
                .desired_rows(5)
                .desired_width(f32::INFINITY),
        );
    }

    /// 設定画面
    fn render_settings_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("環境設定");
        ui.separator();

        egui::Grid::new("settings_grid")
            .num_columns(2)
            .spacing([12.0, 10.0])
            .show(ui, |ui| {
                ui.label("翻訳元言語コード:");
                ui.add(egui::TextEdit::singleline(&mut self.source_lang).desired_width(60.0));
                ui.end_row();

                ui.label("翻訳先言語コード:");
                ui.add(egui::TextEdit::singleline(&mut self.target_lang).desired_width(60.0));
                ui.end_row();

                ui.label("グローバルホットキー:");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.hotkey_str).desired_width(120.0));
                    ui.small("（例: Ctrl+Shift+T, Ctrl+Alt+T）");
                });
                ui.end_row();

                ui.label("アイドル時モデル解放:");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Slider::new(&mut self.idle_unload_secs, 0..=1800)
                            .text("秒 (0で常時保持)"),
                    );
                });
                ui.end_row();

                ui.label("クリップボード監視:");
                ui.checkbox(
                    &mut self.watch_clipboard,
                    "コピーされたテキストを自動翻訳する",
                );
                ui.end_row();

                ui.label("翻訳結果の自動コピー:");
                ui.checkbox(
                    &mut self.copy_result,
                    "翻訳完了時に結果をクリップボードに書き戻す",
                );
                ui.end_row();

                ui.label("文字数制限:");
                ui.horizontal(|ui| {
                    ui.label("最小:");
                    ui.add(egui::DragValue::new(&mut self.min_chars).range(1..=100));
                    ui.label("最大:");
                    ui.add(egui::DragValue::new(&mut self.max_chars).range(100..=50000));
                });
                ui.end_row();
            });

        ui.add_space(12.0);
        if ui.button("設定を保存").clicked() {
            self.update_settings();
        }
    }
}

impl eframe::App for GuiApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_os_events(ui.ctx());
        self.process_events(ui.ctx());
        self.run_ticks(ui.ctx());
        self.copy_pending_result(ui.ctx());

        // ウィンドウが非表示（トレイ格納中）なら CentralPanel を描画しない
        if !self.is_window_visible {
            return;
        }

        egui::CentralPanel::default().show(ui, |ui| {
            // ヘッダーバー
            ui.horizontal(|ui| {
                ui.heading("tray-translate");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.tray.is_some() && ui.button("トレイに隠す").clicked() {
                        ui.ctx()
                            .send_viewport_cmd(egui::ViewportCommand::Visible(false));
                        self.is_window_visible = false;
                    }
                });
            });

            // タブセレクタ
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, ActiveTab::Translate, "翻訳");
                ui.selectable_value(&mut self.active_tab, ActiveTab::Models, "モデル管理");
                ui.selectable_value(&mut self.active_tab, ActiveTab::Settings, "設定");
            });
            ui.separator();

            // 各タブの内容
            match self.active_tab {
                ActiveTab::Translate => self.render_translate_tab(ui),
                ActiveTab::Models => {
                    self.models_view.show(ui, &self.installed, &self.cmd_tx);
                }
                ActiveTab::Settings => self.render_settings_tab(ui),
            }

            ui.add_space(4.0);
            ui.separator();

            // フッター・ステータスバー
            ui.label(&self.status);
            ui.small(format!("導入済みのモデル: {}", self.installed.join(", ")));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coalesces_queued_ticks_into_one_batch() {
        let (tx, rx) = channel();
        tx.send(Command::IdleTick).unwrap();
        tx.send(Command::ClipboardTick).unwrap();
        tx.send(Command::IdleTick).unwrap();

        let batch = coalesce_ticks(&Command::ClipboardTick, &rx);

        assert!(batch.clipboard);
        assert!(batch.idle);
        assert!(batch.next.is_none());
        // 集約したので、キューは空になる
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn single_tick_only_sets_its_own_flag() {
        let (_tx, rx) = channel();

        let clipboard = coalesce_ticks(&Command::ClipboardTick, &rx);
        assert!(clipboard.clipboard);
        assert!(!clipboard.idle);

        let idle = coalesce_ticks(&Command::IdleTick, &rx);
        assert!(!idle.clipboard);
        assert!(idle.idle);
    }

    #[test]
    fn stops_at_first_non_tick_command() {
        let (tx, rx) = channel();
        tx.send(Command::IdleTick).unwrap();
        tx.send(Command::TranslateText("hello".to_string())).unwrap();
        tx.send(Command::ClipboardTick).unwrap();

        let batch = coalesce_ticks(&Command::ClipboardTick, &rx);

        assert!(batch.clipboard);
        assert!(batch.idle);
        assert!(matches!(
            batch.next,
            Some(Command::TranslateText(ref text)) if text == "hello"
        ));
        // 操作より後ろの tick は取り出さず、次の周回で処理する
        assert!(matches!(rx.try_recv(), Ok(Command::ClipboardTick)));
    }
}
