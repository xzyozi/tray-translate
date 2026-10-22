use eframe::egui;
use crate::theme;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use tray_translate_app::Command;
use tray_translate_core::{get_model_dir, ModelHub, PackageIndexEntry};

/// モデル管理画面からの非同期イベント
pub enum ModelManagerEvent {
    IndexFetched(Result<Vec<PackageIndexEntry>, String>),
}

/// モデル管理機能のステート
pub struct ModelsView {
    /// 取得した公式モデルインデックス（取得前は None）
    pub index_entries: Option<Vec<PackageIndexEntry>>,
    /// インデックス取得中フラグ
    pub is_fetching_index: bool,
    /// インデックス取得エラー
    pub index_error: Option<String>,
    /// 絞り込みフィルター文字列
    pub filter: String,
    /// 現在ダウンロード中のペアキー（例: `en_ja`）
    pub active_download_pair: Option<String>,
    /// ダウンロード進捗 (downloaded_bytes, total_bytes)
    pub download_progress: Option<(u64, Option<u64>)>,
    /// 非同期イベント受信チャネル
    event_rx: Receiver<ModelManagerEvent>,
    event_tx: Sender<ModelManagerEvent>,
}

impl ModelsView {
    pub fn new() -> Self {
        let (event_tx, event_rx) = channel();
        Self {
            index_entries: None,
            is_fetching_index: false,
            index_error: None,
            filter: String::new(),
            active_download_pair: None,
            download_progress: None,
            event_rx,
            event_tx,
        }
    }

    /// 非同期イベントを処理する
    pub fn update(&mut self) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                ModelManagerEvent::IndexFetched(result) => {
                    self.is_fetching_index = false;
                    match result {
                        Ok(entries) => {
                            self.index_entries = Some(entries);
                            self.index_error = None;
                        }
                        Err(err) => {
                            self.index_error = Some(err);
                        }
                    }
                }
            }
        }
    }

    /// 公式モデル一覧を別スレッドで取得する（完全オフライン方針：ボタン押下時のみ実行）
    pub fn fetch_index(&mut self, ctx: egui::Context) {
        if self.is_fetching_index {
            return;
        }
        self.is_fetching_index = true;
        self.index_error = None;
        let tx = self.event_tx.clone();

        thread::Builder::new()
            .name("model-index-fetcher".to_string())
            .spawn(move || {
                let res = ModelHub::fetch_index().map_err(|e| {
                    // エラーメッセージからセンシティブ情報や長大なURLを避け、安全な表現にする
                    format!("モデル一覧の取得に失敗しました: {e}")
                });
                let _ = tx.send(ModelManagerEvent::IndexFetched(res));
                ctx.request_repaint();
            })
            .ok();
    }

    /// 指定したパッケージのダウンロードとインストールを別スレッドで開始する
    pub fn start_download(
        &mut self,
        entry: PackageIndexEntry,
        cmd_tx: Sender<Command>,
        ctx: egui::Context,
    ) {
        let pair = format!("{}_{}", entry.from_code, entry.to_code);
        if self.active_download_pair.is_some() {
            return;
        }
        self.active_download_pair = Some(pair.clone());
        self.download_progress = Some((0, None));

        // Controller 側へ開始を通知
        let _ = cmd_tx.send(Command::StartDownload(pair.clone()));

        thread::Builder::new()
            .name("model-downloader".to_string())
            .spawn(move || {
                let target_dir_result = get_model_dir();
                let res = match target_dir_result {
                    Ok(target_dir) => {
                        let cmd_tx_clone = cmd_tx.clone();
                        let ctx_clone = ctx.clone();
                        ModelHub::download_and_install_with_progress(
                            &entry,
                            target_dir,
                            move |downloaded, total| {
                                let _ = cmd_tx_clone.send(Command::DownloadProgress {
                                    downloaded_bytes: downloaded,
                                    total_bytes: total,
                                });
                                ctx_clone.request_repaint();
                            },
                        )
                        .map(|_| ())
                        .map_err(|e| format!("{e}"))
                    }
                    Err(e) => Err(format!("モデル保存先ディレクトリの取得に失敗: {e}")),
                };

                let _ = cmd_tx.send(Command::DownloadFinished(res));
                ctx.request_repaint();
            })
            .ok();
    }

    /// モデル管理 UI を描画する
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        installed_pairs: &[String],
        cmd_tx: &Sender<Command>,
    ) {
        self.update();

        ui.horizontal(|ui| {
            ui.heading("モデル管理");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let fetch_btn_text = if self.is_fetching_index {
                    "一覧取得中…"
                } else {
                    "公式モデル一覧を取得 (通信)"
                };
                if ui
                    .add_enabled(!self.is_fetching_index, egui::Button::new(fetch_btn_text))
                    .clicked()
                {
                    self.fetch_index(ui.ctx().clone());
                }
            });
        });

        ui.label("完全オフライン動作のため、モデル一覧やダウンロードは上記のボタンを押した時のみ通信します。");
        ui.separator();

        // ダウンロード進捗表示
        if let Some(pair) = &self.active_download_pair {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("ダウンロード中: {pair}"));
                    if let Some((downloaded, total_opt)) = self.download_progress {
                        if let Some(total) = total_opt {
                            let ratio = if total > 0 {
                                (downloaded as f32 / total as f32).clamp(0.0, 1.0)
                            } else {
                                0.0
                            };
                            let mb_down = downloaded as f64 / 1_048_576.0;
                            let mb_total = total as f64 / 1_048_576.0;
                            ui.add(egui::ProgressBar::new(ratio).text(format!(
                                "{:.1} MB / {:.1} MB ({:.0}%)",
                                mb_down,
                                mb_total,
                                ratio * 100.0
                            )));
                        } else {
                            let mb_down = downloaded as f64 / 1_048_576.0;
                            ui.label(format!("{:.1} MB 受信済…", mb_down));
                        }
                    }
                });
            });
            ui.add_space(4.0);
        }

        // インデックス取得エラーの表示
        if let Some(err) = &self.index_error {
            ui.colored_label(theme::ERROR, err);
        }

        // フィルター入力
        ui.horizontal(|ui| {
            ui.label("モデル検索:");
            ui.add(
                egui::TextEdit::singleline(&mut self.filter)
                    .hint_text("言語名または言語コード（例: ja, Japanese, en）"),
            );
            if !self.filter.is_empty() && ui.button("クリア").clicked() {
                self.filter.clear();
            }
        });

        ui.add_space(4.0);

        // モデル一覧テーブル
        let mut to_download: Option<PackageIndexEntry> = None;
        egui::ScrollArea::vertical()
            .max_height(280.0)
            .show(ui, |ui| {
                if let Some(entries) = &self.index_entries {
                    let filter_lower = self.filter.trim().to_ascii_lowercase();
                    let filtered_entries: Vec<&PackageIndexEntry> = entries
                        .iter()
                        .filter(|e| {
                            if filter_lower.is_empty() {
                                return true;
                            }
                            e.from_code.to_ascii_lowercase().contains(&filter_lower)
                                || e.to_code.to_ascii_lowercase().contains(&filter_lower)
                                || e.from_name.to_ascii_lowercase().contains(&filter_lower)
                                || e.to_name.to_ascii_lowercase().contains(&filter_lower)
                        })
                        .collect();

                    if filtered_entries.is_empty() {
                        ui.label("一致するモデルがありません。");
                    } else {
                        let is_downloading_any = self.active_download_pair.is_some();
                        let active_pair = self.active_download_pair.clone();

                        egui::Grid::new("models_table")
                            .striped(true)
                            .min_col_width(80.0)
                            .show(ui, |ui| {
                                ui.strong("言語ペア");
                                ui.strong("言語名");
                                ui.strong("版");
                                ui.strong("状態 / 操作");
                                ui.end_row();

                                for entry in filtered_entries {
                                    let pair = format!("{}_{}", entry.from_code, entry.to_code);
                                    let is_installed = installed_pairs.contains(&pair);
                                    let is_downloading =
                                        active_pair.as_deref() == Some(pair.as_str());

                                    ui.label(&pair);
                                    ui.label(format!("{} → {}", entry.from_name, entry.to_name));
                                    ui.label(&entry.package_version);

                                    if is_installed {
                                        ui.colored_label(theme::SUCCESS, "導入済み");
                                    } else if is_downloading {
                                        ui.label("ダウンロード中…");
                                    } else {
                                        let btn_enabled = !is_downloading_any;
                                        if ui
                                            .add_enabled(btn_enabled, egui::Button::new("取得"))
                                            .clicked()
                                        {
                                            to_download = Some(entry.clone());
                                        }
                                    }
                                    ui.end_row();
                                }
                            });
                    }
                } else if self.is_fetching_index {
                    ui.label("公式モデルインデックスを取得しています…");
                } else {
                    ui.label("右上の「公式モデル一覧を取得 (通信)」ボタンを押すと、Argos Translate 公式の利用可能モデル一覧を表示します。");
                    ui.add_space(8.0);
                    ui.strong(format!("現在の導入済みモデル: {}", installed_pairs.join(", ")));
                }
            });

        if let Some(entry) = to_download {
            self.start_download(entry, cmd_tx.clone(), ui.ctx().clone());
        }

        ui.separator();
        ui.small("モデルライセンス表記: Argos Translate Models (Creative Commons Attribution 4.0 International)");
    }
}
