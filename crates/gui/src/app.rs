use crate::clipboard::ArboardClipboard;
use crate::fonts::install_japanese_font;
use eframe::egui;
use std::time::{Duration, Instant};
use tray_translate_app::{get_settings_path, Command, Controller, Event, Settings, SystemClock};
use tray_translate_core::TranslationRouter;

/// クリップボード確認とアイドル解放の確認の間隔
const TICK_INTERVAL: Duration = Duration::from_secs(1);

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// 翻訳ウィンドウ。制御は `Controller` に任せ、ここでは表示と入力の受け渡しだけを行う。
pub struct GuiApp {
    controller: Controller<TranslationRouter, SystemClock>,
    /// 起動時に見つかった導入済みのモデル（ペアキー）
    installed: Vec<String>,
    input: String,
    output: String,
    status: String,
    source_lang: String,
    target_lang: String,
    watch_clipboard: bool,
    copy_result: bool,
    /// 「翻訳中」を表示してから、次のフレームで実行するテキスト
    pending: Option<String>,
    /// 自動コピーする翻訳結果（次のフレームの先頭でクリップボードへ書く）
    to_copy: Option<String>,
    last_tick: Instant,
}

impl GuiApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Result<Self, BoxError> {
        let font = install_japanese_font(&cc.egui_ctx);

        let settings_path = get_settings_path()?;
        let settings = Settings::load(&settings_path)?;
        let router = TranslationRouter::new()?;
        let installed = router.available_pairs();

        let source_lang = settings.source_lang.clone();
        let target_lang = settings.target_lang.clone();
        let watch_clipboard = settings.watch_clipboard;
        let copy_result = settings.copy_result;

        let controller = Controller::new(
            settings,
            Some(settings_path),
            router,
            SystemClock::new(),
            Box::new(ArboardClipboard::default()),
        )?;

        let status = if font.is_none() {
            "日本語フォントが見つかりません（日本語が四角で表示される場合があります）".to_string()
        } else {
            "準備ができました".to_string()
        };

        Ok(Self {
            controller,
            installed,
            input: String::new(),
            output: String::new(),
            status,
            source_lang,
            target_lang,
            watch_clipboard,
            copy_result,
            pending: None,
            to_copy: None,
            last_tick: Instant::now(),
        })
    }

    /// コントローラからの通知を画面の状態へ反映する
    fn apply(&mut self, events: Vec<Event>, from_clipboard: bool) {
        for event in events {
            match event {
                Event::Translated { source, output } => {
                    let mut status = if from_clipboard {
                        self.input = source;
                        "クリップボードの内容を翻訳しました".to_string()
                    } else {
                        "翻訳しました".to_string()
                    };
                    if self.copy_result {
                        self.to_copy = Some(output.clone());
                        status.push_str("（結果をクリップボードにコピーしました）");
                    }
                    self.status = status;
                    self.output = output;
                }
                Event::TextIgnored => {
                    self.status = "文字数が範囲外のため翻訳しませんでした".to_string();
                }
                Event::ModelsUnloaded => {
                    self.status = "しばらく使われなかったため、モデルを解放しました".to_string();
                }
                Event::SettingsUpdated { hotkey_changed: _ } => {
                    self.status = "設定を保存しました".to_string();
                }
                Event::DownloadStarted { pair } => {
                    self.status = format!("ダウンロードを開始しました: {pair}");
                }
                Event::DownloadSucceeded { pair } => {
                    self.status = format!("ダウンロードが完了しました: {pair}");
                }
                Event::DownloadFailed { pair, message } => {
                    self.status = format!("ダウンロードに失敗しました: {pair} ({message})");
                }
                Event::Error { message } => {
                    self.status = format!("エラー: {message}");
                }
            }
        }
    }

    /// 画面の言語・監視の設定を、コントローラへ反映する
    fn update_settings(&mut self) {
        let mut settings = self.controller.settings().clone();
        settings.source_lang = self.source_lang.trim().to_ascii_lowercase();
        settings.target_lang = self.target_lang.trim().to_ascii_lowercase();
        settings.watch_clipboard = self.watch_clipboard;
        settings.copy_result = self.copy_result;

        let events = self.controller.handle(Command::UpdateSettings(settings));
        self.apply(events, false);
    }

    /// 一定間隔で、クリップボードの確認とアイドル解放の確認を行う
    fn run_ticks(&mut self, ctx: &egui::Context) {
        if self.last_tick.elapsed() >= TICK_INTERVAL {
            self.last_tick = Instant::now();

            let events = self.controller.handle(Command::ClipboardTick);
            self.apply(events, true);

            let events = self.controller.handle(Command::IdleTick);
            self.apply(events, false);
        }
        // 入力が無くても、次の確認のために再描画を予約する
        ctx.request_repaint_after(TICK_INTERVAL);
    }

    /// 自動コピーの対象があれば、クリップボードへ書く。
    /// 書いた内容は、クリップボード監視が再翻訳しないように既読にする。
    fn copy_pending_result(&mut self, ctx: &egui::Context) {
        if let Some(text) = self.to_copy.take() {
            self.controller.note_clipboard_written(&text);
            ctx.copy_text(text);
        }
    }

    fn language_row(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("翻訳元");
            ui.add(egui::TextEdit::singleline(&mut self.source_lang).desired_width(48.0));
            ui.label("→ 翻訳先");
            ui.add(egui::TextEdit::singleline(&mut self.target_lang).desired_width(48.0));
            if ui.button("適用").clicked() {
                self.update_settings();
            }
        });

        let label = "クリップボードを監視して自動翻訳する";
        if ui.checkbox(&mut self.watch_clipboard, label).changed() {
            self.update_settings();
        }

        let label = "翻訳結果を自動でクリップボードにコピーする";
        if ui.checkbox(&mut self.copy_result, label).changed() {
            self.update_settings();
        }

        let missing = self.controller.missing_packages(&self.installed);
        if !missing.is_empty() {
            let text = format!(
                "未導入のモデル: {}（`tt download` で導入してください）",
                missing.join(", ")
            );
            ui.colored_label(egui::Color32::from_rgb(200, 120, 0), text);
        }
    }
}

impl eframe::App for GuiApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // 前のフレームで予約された翻訳を実行する（実行中は画面が止まる）
        if let Some(text) = self.pending.take() {
            let events = self.controller.handle(Command::TranslateText(text));
            self.apply(events, false);
        }
        self.run_ticks(ui.ctx());
        self.copy_pending_result(ui.ctx());

        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("tray-translate");
            ui.separator();

            self.language_row(ui);
            ui.add_space(8.0);

            ui.label("翻訳するテキスト");
            ui.add(
                egui::TextEdit::multiline(&mut self.input)
                    .desired_rows(6)
                    .desired_width(f32::INFINITY),
            );

            ui.horizontal(|ui| {
                if ui.button("翻訳").clicked() {
                    self.status = "翻訳中…（初回はモデルの読み込みに時間がかかります）".to_string();
                    self.pending = Some(self.input.clone());
                    ui.ctx().request_repaint();
                }
                if ui.button("結果をコピー").clicked() {
                    self.controller.note_clipboard_written(&self.output);
                    ui.ctx().copy_text(self.output.clone());
                    self.status = "結果をクリップボードにコピーしました".to_string();
                }
                if ui.button("クリア").clicked() {
                    self.input.clear();
                    self.output.clear();
                }
            });

            ui.add_space(8.0);
            ui.label("翻訳結果");
            ui.add(
                egui::TextEdit::multiline(&mut self.output.as_str())
                    .desired_rows(6)
                    .desired_width(f32::INFINITY),
            );

            ui.separator();
            ui.label(&self.status);
            ui.small(format!("導入済みのモデル: {}", self.installed.join(", ")));
        });
    }
}
