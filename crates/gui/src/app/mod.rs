//! 翻訳ウィンドウ本体。状態・言語・描画・ワーカーは子モジュールに分け、ここには
//! `GuiApp`（起動、OS イベント、コントローラとのやり取り）を置く。

mod languages;
mod state;
mod view;
mod worker;

use crate::clipboard::ArboardClipboard;
use crate::fonts::{install_japanese_font, missing_font_message};
use crate::hotkey::HotkeyManager;
use crate::lifecycle::{can_hide_to_tray, on_close_requested, CloseAction};
use crate::models_view::ModelsView;
use crate::theme;
use crate::tray::{TrayAction, TrayManager};
use eframe::egui;
use state::{ActiveTab, AppStatus, HistoryHold, RequestTracker};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};
use tray_translate_app::{
    get_history_path, get_settings_path, models::required_packages, Command, Controller, Event,
    HistoryEntry, Settings, SystemClock,
};
use tray_translate_core::TranslationRouter;
use worker::coalesce_ticks;

/// クリップボード確認とアイドル解放の確認の間隔
const TICK_INTERVAL: Duration = Duration::from_secs(1);

type BoxError = Box<dyn std::error::Error + Send + Sync>;

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
    status: AppStatus,
    source_lang: String,
    target_lang: String,
    hotkey_str: String,
    idle_unload_secs: u64,
    watch_clipboard: bool,
    copy_result: bool,
    min_chars: usize,
    max_chars: usize,
    /// 入力後に自動翻訳する（ADR-006）
    auto_translate: bool,
    /// 入力後自動翻訳のデバウンス待機時間（ミリ秒、ADR-006）
    auto_translate_delay_ms: u64,
    /// 「×」でトレイに常駐するか（ADR-007）。トレイアイコンの作成は起動時のみ行う
    use_tray: bool,
    /// 翻訳リクエスト追跡（古い結果の破棄用、ADR-006）
    request_tracker: RequestTracker,
    /// 自動翻訳のデバウンスタイマー
    pending_auto_translate: Option<Instant>,
    /// 翻訳処理中かどうかのフラグ
    is_translating: bool,
    /// アプリ終了シーケンス中かどうかのフラグ（CancelClose を抑制する）
    is_quitting: bool,
    /// 自動コピーする翻訳結果（次のフレームの先頭でクリップボードへ書く）
    to_copy: Option<String>,
    last_tick: Instant,
    /// 直近にコピーが成功した時刻（コピーボタンの ✓ 表示用）
    last_copied_at: Option<Instant>,
    /// 翻訳履歴（新しい順。制御層から通知される内容の写し、Issue #89）
    history: Vec<HistoryEntry>,
    /// 履歴の上限件数（設定。0 は記録しない）
    history_max_entries: usize,
    /// 自動翻訳の結果を履歴に確定するまでの待機時間（ミリ秒、設定）
    history_commit_delay_ms: u64,
    /// 履歴に確定するまで保留している、自動翻訳の結果
    history_hold: HistoryHold,
    /// 履歴の全消去の確認中か
    confirm_clear_history: bool,
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
        let auto_translate = settings.auto_translate;
        let auto_translate_delay_ms = settings.auto_translate_delay_ms;
        let use_tray = settings.use_tray;
        let history_max_entries = settings.history_max_entries;
        let history_commit_delay_ms = settings.history_commit_delay_ms;

        // 履歴ファイルが壊れていても、空の履歴で起動する（Issue #89）
        let mut controller = Controller::new(
            settings,
            Some(settings_path),
            router,
            SystemClock::new(),
            Box::new(ArboardClipboard::default()),
        )?
        .with_history(Some(get_history_path()?));
        let history = controller.history().entries().to_vec();

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

        // トレイアイコンの初期化（設定で無効なら作らない。失敗しても通常ウィンドウとして動作）
        let tray = if use_tray {
            match TrayManager::new(cc.egui_ctx.clone()) {
                Ok(t) => Some(t),
                Err(e) => {
                    eprintln!("トレイアイコンの初期化をスキップしました: {e}");
                    None
                }
            }
        } else {
            None
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

        let status = match &font {
            Some(path) => {
                eprintln!("日本語フォントを読み込みました: {path}");
                AppStatus::Info("準備ができました".to_string())
            }
            None => {
                let message = missing_font_message();
                eprintln!("{message}");
                AppStatus::Error(message)
            }
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
            auto_translate,
            auto_translate_delay_ms,
            use_tray,
            request_tracker: RequestTracker::new(),
            pending_auto_translate: None,
            is_translating: false,
            is_quitting: false,
            to_copy: None,
            last_tick: Instant::now(),
            last_copied_at: None,
            history,
            history_max_entries,
            history_commit_delay_ms,
            history_hold: HistoryHold::default(),
            confirm_clear_history: false,
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

        // 3. ウィンドウの「×」ボタン押下をインターセプト（判断は lifecycle に集約、ADR-007）
        if ctx.input(|i| i.viewport().close_requested()) {
            match on_close_requested(self.use_tray, self.tray.is_some(), self.is_quitting) {
                CloseAction::HideToTray => {
                    // 終了せずにウィンドウを隠す
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                    self.is_window_visible = false;
                }
                // そのまま閉じる（プロセスを終了する）
                CloseAction::Quit => {}
            }
        }
    }

    /// コントローラからの通知を画面の状態へ反映する
    fn process_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                Event::Translated { source, output } => {
                    self.is_translating = false;
                    let (accept, is_auto) = self.request_tracker.on_translated(&source);
                    if !accept {
                        eprintln!("古い翻訳結果（source: {source:?}）を破棄しました");
                        continue;
                    }

                    // 採用された結果を履歴に残す（自動翻訳の結果は確定まで保留する、Issue #89）
                    let history_entry = HistoryEntry::new(
                        self.source_lang.trim().to_ascii_lowercase(),
                        self.target_lang.trim().to_ascii_lowercase(),
                        source.trim().to_string(),
                        output.clone(),
                    );

                    let mut status = "翻訳しました".to_string();
                    if self.input.trim() != source.trim() {
                        self.input = source;
                        status = "クリップボードの内容を翻訳しました".to_string();
                    }
                    // 自動翻訳時はクリップボード自動コピーを行わない（ADR-006）
                    if !is_auto && self.copy_result {
                        self.to_copy = Some(output.clone());
                        status.push_str("（結果をクリップボードにコピーしました）");
                    }
                    self.status = AppStatus::Info(status);
                    self.output = output;

                    if is_auto {
                        self.hold_history(history_entry);
                    } else {
                        self.commit_history(history_entry);
                    }
                }
                Event::TextIgnored => {
                    self.is_translating = false;
                    self.status =
                        AppStatus::Info("文字数が範囲外のため翻訳しませんでした".to_string());
                }
                Event::ModelsUnloaded => {
                    self.status = AppStatus::Info(
                        "しばらく使われなかったため、モデルを解放しました".to_string(),
                    );
                }
                Event::SettingsUpdated { hotkey_changed } => {
                    if hotkey_changed {
                        if let Some(hotkey) = &mut self.hotkey {
                            if let Err(e) = hotkey.register(&self.hotkey_str) {
                                self.status = AppStatus::Error(format!(
                                    "ホットキー再登録失敗（旧キーを維持）: {e}"
                                ));
                            } else {
                                self.status =
                                    AppStatus::Info("設定とホットキーを更新しました".to_string());
                            }
                        }
                    } else {
                        self.status = AppStatus::Info("設定を保存しました".to_string());
                    }
                }
                Event::DownloadStarted { pair } => {
                    // コントローラが受理した時点で、ダウンロード用のスレッドを起動する
                    self.models_view
                        .on_download_started(&pair, self.cmd_tx.clone(), ctx.clone());
                    self.status = AppStatus::Busy(format!("ダウンロードを開始しました: {pair}"));
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
                        self.status =
                            AppStatus::Busy(format!("ダウンロード中: {pair} ({pct:.0}%)"));
                    } else {
                        let mb = downloaded_bytes as f64 / 1_048_576.0;
                        self.status =
                            AppStatus::Busy(format!("ダウンロード中: {pair} ({mb:.1} MB)"));
                    }
                }
                Event::DownloadSucceeded { pair } => {
                    self.models_view.clear_download();
                    self.status = AppStatus::Info(format!("ダウンロードが完了しました: {pair}"));
                }
                Event::DownloadFailed { pair, message } => {
                    self.models_view.clear_download();
                    self.status =
                        AppStatus::Error(format!("ダウンロードに失敗しました: {pair} ({message})"));
                }
                Event::ModelsRefreshed { available_pairs } => {
                    self.installed = available_pairs;
                }
                Event::HistoryUpdated { entries } => {
                    self.history = entries;
                }
                Event::Error { message } => {
                    self.is_translating = false;
                    // ダウンロード開始の要求が拒否された場合は、待機を取り消す
                    self.models_view.on_download_rejected();
                    self.status = AppStatus::Error(format!("エラー: {message}"));
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
            auto_translate: self.auto_translate,
            auto_translate_delay_ms: self.auto_translate_delay_ms,
            use_tray: self.use_tray,
            history_max_entries: self.history_max_entries,
            history_commit_delay_ms: self.history_commit_delay_ms,
        };

        let _ = self.cmd_tx.send(Command::UpdateSettings(settings));
    }

    /// 履歴に1件記録するよう、制御層へ依頼する（書き込みは制御層だけが行う）
    fn commit_history(&mut self, entry: HistoryEntry) {
        let _ = self.cmd_tx.send(Command::RecordHistory(entry));
    }

    /// 保留中の自動翻訳の結果があれば、履歴に確定する
    fn commit_pending_history(&mut self) {
        if let Some(entry) = self.history_hold.take() {
            self.commit_history(entry);
        }
    }

    /// 自動翻訳の結果を保留する。直前の保留が残っていれば、入力は編集されていないので確定する
    fn hold_history(&mut self, entry: HistoryEntry) {
        if let Some(previous) = self.history_hold.hold(entry, Instant::now()) {
            self.commit_history(previous);
        }
    }

    /// 保留中の結果が、待機時間を過ぎていれば履歴に確定する
    fn check_pending_history(&mut self, ctx: &egui::Context) {
        let delay = Duration::from_millis(self.history_commit_delay_ms);
        let now = Instant::now();
        if let Some(entry) = self.history_hold.take_if_due(now, delay) {
            self.commit_history(entry);
        } else if let Some(remaining) = self.history_hold.remaining(now, delay) {
            ctx.request_repaint_after(remaining);
        }
    }

    /// 履歴の1件を画面に戻す。再翻訳はしない
    fn restore_history(&mut self, entry: &HistoryEntry) {
        // 別の操作が始まるので、保留中の結果は確定する
        self.commit_pending_history();

        self.source_lang = entry.source_lang.clone();
        self.target_lang = entry.target_lang.clone();
        self.input = entry.source.clone();
        self.output = entry.output.clone();

        // 古い翻訳結果が復元内容を上書きしないようにし、自動翻訳も起動しない
        self.request_tracker.invalidate();
        self.pending_auto_translate = None;
        self.is_translating = false;

        self.update_settings();
        self.status = AppStatus::Info("履歴から復元しました".to_string());
    }

    /// 翻訳処理を要求する（手動または自動翻訳、ADR-006）
    fn trigger_translation(&mut self, is_auto: bool) {
        let text = self.input.trim().to_string();
        if text.is_empty() {
            return;
        }

        // 手動の翻訳は別の操作なので、保留中の自動翻訳の結果は確定する
        if !is_auto {
            self.commit_pending_history();
        }

        self.pending_auto_translate = None;
        let _ = self
            .request_tracker
            .issue_request(self.input.clone(), is_auto);

        self.update_settings();
        self.status =
            AppStatus::Busy("翻訳中…（初回はモデルの読み込みに時間がかかります）".to_string());
        self.is_translating = true;
        let _ = self.cmd_tx.send(Command::TranslateText(self.input.clone()));
    }

    /// 入力後の自動翻訳デバウンス判定（ADR-006 / Issue #60）
    fn check_auto_translate(&mut self, ctx: &egui::Context) {
        if let Some(started_at) = self.pending_auto_translate {
            if !self.auto_translate || self.input.trim().is_empty() {
                self.pending_auto_translate = None;
            } else {
                let delay = Duration::from_millis(self.auto_translate_delay_ms);
                let elapsed = started_at.elapsed();
                if elapsed >= delay {
                    self.pending_auto_translate = None;
                    self.trigger_translation(true);
                } else {
                    let remaining = delay - elapsed;
                    ctx.request_repaint_after(remaining);
                }
            }
        }
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
            self.last_copied_at = Some(Instant::now());
        }
    }

    fn missing_packages(&self) -> Vec<String> {
        let from = self.source_lang.trim().to_ascii_lowercase();
        let to = self.target_lang.trim().to_ascii_lowercase();
        required_packages(&self.installed, &from, &to)
    }
}

/// 終了時に、確定待ちの自動翻訳の結果を履歴へ書き出す（取りこぼし防止、Issue #89）。
///
/// 書き込みは制御層（ワーカー）だけが行うため、依頼を送ったあと、その応答を短時間だけ待つ。
impl Drop for GuiApp {
    fn drop(&mut self) {
        let Some(entry) = self.history_hold.take() else {
            return;
        };
        if self.cmd_tx.send(Command::RecordHistory(entry)).is_err() {
            return;
        }

        let deadline = Instant::now() + Duration::from_secs(2);
        while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
            match self.event_rx.recv_timeout(remaining) {
                Ok(Event::HistoryUpdated { .. }) | Ok(Event::Error { .. }) | Err(_) => break,
                Ok(_) => {}
            }
        }
    }
}

impl eframe::App for GuiApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_os_events(ui.ctx());
        self.process_events(ui.ctx());
        self.run_ticks(ui.ctx());
        self.check_auto_translate(ui.ctx());
        self.check_pending_history(ui.ctx());
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
                    if ui.button("終了").clicked() {
                        self.is_quitting = true;
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if can_hide_to_tray(self.use_tray, self.tray.is_some())
                        && ui.button("トレイに隠す").clicked()
                    {
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

            // フッター・ステータスバー（ADR-006 / Issue #61）
            self.render_footer(ui);
        });
    }
}
