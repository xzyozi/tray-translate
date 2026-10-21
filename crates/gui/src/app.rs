use crate::clipboard::ArboardClipboard;
use crate::fonts::install_japanese_font;
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

/// 翻訳ウィンドウ。制御は別スレッドの `Controller` に任せ、ここでは表示と入力の受け渡しだけを行う。
pub struct GuiApp {
    cmd_tx: Sender<Command>,
    event_rx: Receiver<Event>,
    /// 起動時に見つかった導入済みのモデル（ペアキー）
    installed: Vec<String>,
    input: String,
    output: String,
    status: String,
    source_lang: String,
    target_lang: String,
    watch_clipboard: bool,
    copy_result: bool,
    /// 翻訳処理中かどうかのフラグ
    is_translating: bool,
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
                while let Ok(cmd) = cmd_rx.recv() {
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

        let status = if font.is_none() {
            "日本語フォントが見つかりません（日本語が四角で表示される場合があります）".to_string()
        } else {
            "準備ができました".to_string()
        };

        Ok(Self {
            cmd_tx,
            event_rx,
            installed,
            input: String::new(),
            output: String::new(),
            status,
            source_lang,
            target_lang,
            watch_clipboard,
            copy_result,
            is_translating: false,
            to_copy: None,
            last_tick: Instant::now(),
        })
    }

    /// コントローラからの通知を画面の状態へ反映する
    fn process_events(&mut self) {
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
                    self.is_translating = false;
                    self.status = format!("エラー: {message}");
                }
            }
        }
    }

    /// 画面の言語・監視の設定を、コントローラへ反映する
    fn update_settings(&mut self) {
        let settings = Settings {
            source_lang: self.source_lang.trim().to_ascii_lowercase(),
            target_lang: self.target_lang.trim().to_ascii_lowercase(),
            watch_clipboard: self.watch_clipboard,
            copy_result: self.copy_result,
            ..Settings::default()
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
        // 入力が無くても、次の確認のために再描画を予約する
        ctx.request_repaint_after(TICK_INTERVAL);
    }

    /// 自動コピーの対象があれば、クリップボードへ書く。
    /// 書いた内容は、クリップボード監視が再翻訳しないように既読にする。
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

        let missing = self.missing_packages();
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
        self.process_events();
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
                let btn_text = if self.is_translating {
                    "翻訳中…"
                } else {
                    "翻訳"
                };
                let btn = ui.add_enabled(!self.is_translating, egui::Button::new(btn_text));
                if btn.clicked() {
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
