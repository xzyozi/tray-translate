use crate::clipboard::{ClipboardSource, ClipboardWatcher};
use crate::config::Settings;
use crate::error::Result;
use crate::hotkey::Hotkey;
use crate::models::{parse_pair_key, required_packages, DownloadState, DownloadTracker};
use crate::service::{Clock, ServiceConfig, Translate, TranslationService};
use std::path::PathBuf;

/// 画面や OS 連携の側から、コントローラへ渡す操作
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// ユーザーが入力・選択したテキストを翻訳する
    TranslateText(String),
    /// クリップボードを確認する（`watch_clipboard` が有効なら自動翻訳する）
    ClipboardTick,
    /// アイドル時のモデル解放を確認する
    IdleTick,
    /// 設定を更新する（検証し、保存先があれば保存する）
    UpdateSettings(Settings),
    /// モデルのダウンロード開始を記録する（`en_ja` 形式のペアキー）
    StartDownload(String),
    /// ダウンロードの終了を記録する（失敗時はメッセージ）
    DownloadFinished(std::result::Result<(), String>),
    /// アプリ自身が書き込んだテキストを既読にする（自分の出力を再翻訳しないため）
    NoteClipboardWritten(String),
}

/// コントローラから、画面や OS 連携の側へ返す通知
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Translated {
        source: String,
        output: String,
    },
    /// 文字数が範囲外のため翻訳しなかった（ユーザー操作の場合のみ通知する）
    TextIgnored,
    ModelsUnloaded,
    SettingsUpdated {
        /// ホットキーが変わった。OS 側の登録をやり直す必要がある
        hotkey_changed: bool,
    },
    DownloadStarted {
        pair: String,
    },
    DownloadSucceeded {
        pair: String,
    },
    DownloadFailed {
        pair: String,
        message: String,
    },
    Error {
        message: String,
    },
}

/// 常駐アプリの制御。画面・トレイ・ホットキー・実クリップボードは、このコントローラの外側に置き、
/// `Command` を渡して `Event` を受け取る形でつなぐ。
pub struct Controller<T: Translate, C: Clock> {
    settings: Settings,
    settings_path: Option<PathBuf>,
    service: TranslationService<T, C>,
    clipboard: Box<dyn ClipboardSource>,
    watcher: ClipboardWatcher,
    downloads: DownloadTracker,
}

impl<T: Translate, C: Clock> Controller<T, C> {
    /// コントローラを作る。設定が不正ならエラー。
    /// 起動時にクリップボードにあった内容は、翻訳対象にしない。
    pub fn new(
        settings: Settings,
        settings_path: Option<PathBuf>,
        backend: T,
        clock: C,
        mut clipboard: Box<dyn ClipboardSource>,
    ) -> Result<Self> {
        settings.validate()?;

        let service = TranslationService::new(backend, clock, ServiceConfig::from(&settings));
        let mut watcher = ClipboardWatcher::new(settings.min_chars, settings.max_chars);
        watcher.prime(clipboard.as_mut());

        Ok(Self {
            settings,
            settings_path,
            service,
            clipboard,
            watcher,
            downloads: DownloadTracker::new(),
        })
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn download_state(&self) -> &DownloadState {
        self.downloads.state()
    }

    /// 現在の言語設定で翻訳するために、まだ導入されていないパッケージのペアキー
    pub fn missing_packages(&self, installed: &[String]) -> Vec<String> {
        required_packages(
            installed,
            &self.settings.source_lang,
            &self.settings.target_lang,
        )
    }

    /// アプリが書き込んだテキストを既読にする（自分の出力を再翻訳しないため）
    pub fn note_clipboard_written(&mut self, text: &str) {
        self.watcher.mark_written(text);
    }

    /// 操作を処理し、結果の通知を返す
    pub fn handle(&mut self, command: Command) -> Vec<Event> {
        match command {
            Command::TranslateText(text) => self.translate(&text, true),
            Command::ClipboardTick => self.clipboard_tick(),
            Command::IdleTick => self.idle_tick(),
            Command::UpdateSettings(settings) => self.update_settings(settings),
            Command::StartDownload(pair) => self.start_download(&pair),
            Command::DownloadFinished(result) => self.finish_download(result),
            Command::NoteClipboardWritten(text) => {
                self.note_clipboard_written(&text);
                Vec::new()
            }
        }
    }

    fn translate(&mut self, text: &str, report_ignored: bool) -> Vec<Event> {
        match self.service.translate(text) {
            Ok(Some(output)) => vec![Event::Translated {
                source: text.trim().to_string(),
                output,
            }],
            Ok(None) if report_ignored => vec![Event::TextIgnored],
            Ok(None) => Vec::new(),
            Err(error) => vec![error_event(error.to_string())],
        }
    }

    fn clipboard_tick(&mut self) -> Vec<Event> {
        if !self.settings.watch_clipboard {
            return Vec::new();
        }
        match self.watcher.poll(self.clipboard.as_mut()) {
            Some(text) => self.translate(&text, false),
            None => Vec::new(),
        }
    }

    fn idle_tick(&mut self) -> Vec<Event> {
        if self.service.unload_if_idle() {
            vec![Event::ModelsUnloaded]
        } else {
            Vec::new()
        }
    }

    fn update_settings(&mut self, new: Settings) -> Vec<Event> {
        if let Err(error) = new.validate() {
            return vec![error_event(error.to_string())];
        }
        if let Some(path) = &self.settings_path {
            if let Err(error) = new.save(path) {
                return vec![error_event(error.to_string())];
            }
        }

        let old_hotkey = Hotkey::parse(&self.settings.hotkey).ok();
        let new_hotkey = Hotkey::parse(&new.hotkey).ok();
        let hotkey_changed = old_hotkey != new_hotkey;

        self.service.set_config(ServiceConfig::from(&new));
        // 文字数の範囲が変わるため作り直す。監視を有効にした時点の内容は翻訳対象にしない。
        self.watcher = ClipboardWatcher::new(new.min_chars, new.max_chars);
        self.watcher.prime(self.clipboard.as_mut());
        self.settings = new;

        vec![Event::SettingsUpdated { hotkey_changed }]
    }

    fn start_download(&mut self, pair: &str) -> Vec<Event> {
        if parse_pair_key(pair).is_none() {
            return vec![error_event(format!("invalid package pair: {pair:?}"))];
        }
        match self.downloads.start(pair) {
            Ok(()) => vec![Event::DownloadStarted {
                pair: pair.to_string(),
            }],
            Err(error) => vec![error_event(error.to_string())],
        }
    }

    fn finish_download(&mut self, result: std::result::Result<(), String>) -> Vec<Event> {
        let pair = match self.downloads.state() {
            DownloadState::Running { pair } => pair.clone(),
            _ => return vec![error_event("no download is in progress".to_string())],
        };

        match result {
            Ok(()) => {
                self.downloads.finish_ok();
                vec![Event::DownloadSucceeded { pair }]
            }
            Err(message) => {
                self.downloads.finish_err(&message);
                vec![Event::DownloadFailed { pair, message }]
            }
        }
    }
}

fn error_event(message: String) -> Event {
    Event::Error { message }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;
    use std::time::Duration;

    struct FakeBackend {
        loaded: usize,
    }

    impl Translate for FakeBackend {
        fn translate(&mut self, from: &str, to: &str, text: &str) -> Result<String> {
            self.loaded = 1;
            Ok(format!("[{from}->{to}] {text}"))
        }

        fn unload_all(&mut self) {
            self.loaded = 0;
        }

        fn loaded_count(&self) -> usize {
            self.loaded
        }
    }

    use std::sync::{Arc, Mutex};

    struct FakeClock(Rc<Cell<Duration>>);

    impl Clock for FakeClock {
        fn now(&self) -> Duration {
            self.0.get()
        }
    }

    struct SharedClipboard(Arc<Mutex<Option<String>>>);

    impl ClipboardSource for SharedClipboard {
        fn read_text(&mut self) -> Option<String> {
            self.0.lock().unwrap().clone()
        }
    }

    struct Fixture {
        controller: Controller<FakeBackend, FakeClock>,
        time: Rc<Cell<Duration>>,
        clipboard: Arc<Mutex<Option<String>>>,
    }

    impl Fixture {
        fn set_clipboard(&self, text: &str) {
            *self.clipboard.lock().unwrap() = Some(text.to_string());
        }
    }

    fn base_settings() -> Settings {
        Settings {
            idle_unload_secs: 10,
            watch_clipboard: true,
            min_chars: 2,
            max_chars: 20,
            ..Settings::default()
        }
    }

    fn fixture_with(
        settings: Settings,
        settings_path: Option<PathBuf>,
        initial_clipboard: Option<&str>,
    ) -> Fixture {
        let time = Rc::new(Cell::new(Duration::ZERO));
        let clipboard = Arc::new(Mutex::new(initial_clipboard.map(|t| t.to_string())));
        let controller = Controller::new(
            settings,
            settings_path,
            FakeBackend { loaded: 0 },
            FakeClock(Rc::clone(&time)),
            Box::new(SharedClipboard(Arc::clone(&clipboard))),
        )
        .unwrap();
        Fixture {
            controller,
            time,
            clipboard,
        }
    }

    fn fixture() -> Fixture {
        fixture_with(base_settings(), None, None)
    }

    fn translated(source: &str, output: &str) -> Event {
        Event::Translated {
            source: source.to_string(),
            output: output.to_string(),
        }
    }

    fn is_error_containing(event: &Event, needle: &str) -> bool {
        matches!(event, Event::Error { message } if message.contains(needle))
    }

    #[test]
    fn invalid_settings_are_rejected_at_construction() {
        let settings = Settings {
            hotkey: "T".to_string(),
            ..base_settings()
        };
        let result = Controller::new(
            settings,
            None,
            FakeBackend { loaded: 0 },
            FakeClock(Rc::new(Cell::new(Duration::ZERO))),
            Box::new(SharedClipboard(Arc::new(Mutex::new(None)))),
        );
        assert!(result.is_err());
    }

    #[test]
    fn translates_user_text() {
        let mut f = fixture();
        let events = f
            .controller
            .handle(Command::TranslateText("  hello ".to_string()));
        assert_eq!(events, vec![translated("hello", "[en->ja] hello")]);
    }

    #[test]
    fn short_user_text_is_reported_as_ignored() {
        let mut f = fixture();
        let events = f.controller.handle(Command::TranslateText("a".to_string()));
        assert_eq!(events, vec![Event::TextIgnored]);
    }

    #[test]
    fn too_long_user_text_is_reported_as_error() {
        let mut f = fixture();
        let text = "x".repeat(21);
        let events = f.controller.handle(Command::TranslateText(text));
        assert_eq!(events.len(), 1);
        assert!(is_error_containing(&events[0], "too long"));
    }

    #[test]
    fn clipboard_content_at_startup_is_not_translated() {
        let mut f = fixture_with(base_settings(), None, Some("existing"));
        assert!(f.controller.handle(Command::ClipboardTick).is_empty());
    }

    #[test]
    fn new_clipboard_content_is_translated_once() {
        let mut f = fixture_with(base_settings(), None, Some("existing"));

        f.set_clipboard("new text");
        let events = f.controller.handle(Command::ClipboardTick);
        assert_eq!(events, vec![translated("new text", "[en->ja] new text")]);
        assert!(f.controller.handle(Command::ClipboardTick).is_empty());
    }

    #[test]
    fn clipboard_is_ignored_when_watching_is_disabled() {
        let settings = Settings {
            watch_clipboard: false,
            ..base_settings()
        };
        let mut f = fixture_with(settings, None, None);

        f.set_clipboard("new text");
        assert!(f.controller.handle(Command::ClipboardTick).is_empty());
    }

    #[test]
    fn out_of_range_clipboard_text_is_silently_ignored() {
        let mut f = fixture();
        f.set_clipboard("a");
        assert!(f.controller.handle(Command::ClipboardTick).is_empty());
    }

    #[test]
    fn own_clipboard_output_is_not_translated_again() {
        let mut f = fixture();

        f.set_clipboard("hello");
        f.controller.handle(Command::ClipboardTick);

        f.controller.note_clipboard_written("translated");
        f.set_clipboard("translated");
        assert!(f.controller.handle(Command::ClipboardTick).is_empty());
    }

    #[test]
    fn idle_models_are_unloaded_once() {
        let mut f = fixture();
        f.controller
            .handle(Command::TranslateText("hello".to_string()));

        f.time.set(Duration::from_secs(9));
        assert!(f.controller.handle(Command::IdleTick).is_empty());

        f.time.set(Duration::from_secs(10));
        assert_eq!(
            f.controller.handle(Command::IdleTick),
            vec![Event::ModelsUnloaded]
        );
        assert!(f.controller.handle(Command::IdleTick).is_empty());
    }

    #[test]
    fn settings_update_applies_to_next_translation() {
        let mut f = fixture();
        let new = Settings {
            target_lang: "es".to_string(),
            ..base_settings()
        };

        let events = f.controller.handle(Command::UpdateSettings(new.clone()));
        assert_eq!(
            events,
            vec![Event::SettingsUpdated {
                hotkey_changed: false
            }]
        );
        assert_eq!(f.controller.settings(), &new);

        let events = f
            .controller
            .handle(Command::TranslateText("hello".to_string()));
        assert_eq!(events, vec![translated("hello", "[en->es] hello")]);
    }

    #[test]
    fn hotkey_change_is_reported() {
        let mut f = fixture();
        let new = Settings {
            hotkey: "Ctrl+Shift+F5".to_string(),
            ..base_settings()
        };
        let events = f.controller.handle(Command::UpdateSettings(new));
        assert_eq!(
            events,
            vec![Event::SettingsUpdated {
                hotkey_changed: true
            }]
        );
    }

    #[test]
    fn equivalent_hotkey_spelling_is_not_a_change() {
        let mut f = fixture();
        let new = Settings {
            hotkey: "alt+ctrl+t".to_string(),
            ..base_settings()
        };
        let events = f.controller.handle(Command::UpdateSettings(new));
        assert_eq!(
            events,
            vec![Event::SettingsUpdated {
                hotkey_changed: false
            }]
        );
    }

    #[test]
    fn invalid_settings_update_changes_nothing() {
        let mut f = fixture();
        let before = f.controller.settings().clone();
        let bad = Settings {
            min_chars: 0,
            ..base_settings()
        };

        let events = f.controller.handle(Command::UpdateSettings(bad));
        assert_eq!(events.len(), 1);
        assert!(is_error_containing(&events[0], "min_chars"));
        assert_eq!(f.controller.settings(), &before);
    }

    #[test]
    fn settings_update_is_saved_to_the_given_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut f = fixture_with(base_settings(), Some(path.clone()), None);

        let new = Settings {
            target_lang: "fr".to_string(),
            ..base_settings()
        };
        f.controller.handle(Command::UpdateSettings(new.clone()));

        assert_eq!(Settings::load(&path).unwrap(), new);
    }

    #[test]
    fn failed_save_does_not_apply_the_new_settings() {
        let dir = tempfile::tempdir().unwrap();
        // 親がファイルなので、保存先ディレクトリを作れない
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, "file").unwrap();
        let path = blocker.join("settings.json");
        let mut f = fixture_with(base_settings(), Some(path), None);

        let before = f.controller.settings().clone();
        let new = Settings {
            target_lang: "fr".to_string(),
            ..base_settings()
        };
        let events = f.controller.handle(Command::UpdateSettings(new));

        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], Event::Error { .. }));
        assert_eq!(f.controller.settings(), &before);
    }

    #[test]
    fn enabling_clipboard_watching_does_not_translate_existing_content() {
        let settings = Settings {
            watch_clipboard: false,
            ..base_settings()
        };
        let mut f = fixture_with(settings, None, Some("stale"));
        assert!(f.controller.handle(Command::ClipboardTick).is_empty());

        let enabled = Settings {
            watch_clipboard: true,
            ..base_settings()
        };
        f.controller.handle(Command::UpdateSettings(enabled));
        assert!(f.controller.handle(Command::ClipboardTick).is_empty());

        f.set_clipboard("fresh text");
        let events = f.controller.handle(Command::ClipboardTick);
        assert_eq!(
            events,
            vec![translated("fresh text", "[en->ja] fresh text")]
        );
    }

    #[test]
    fn download_success_flow() {
        let mut f = fixture();

        let events = f
            .controller
            .handle(Command::StartDownload("en_ja".to_string()));
        assert_eq!(
            events,
            vec![Event::DownloadStarted {
                pair: "en_ja".to_string()
            }]
        );

        let events = f.controller.handle(Command::DownloadFinished(Ok(())));
        assert_eq!(
            events,
            vec![Event::DownloadSucceeded {
                pair: "en_ja".to_string()
            }]
        );
        assert_eq!(
            f.controller.download_state(),
            &DownloadState::Succeeded {
                pair: "en_ja".to_string()
            }
        );
    }

    #[test]
    fn download_failure_flow() {
        let mut f = fixture();
        f.controller
            .handle(Command::StartDownload("en_ja".to_string()));

        let events = f
            .controller
            .handle(Command::DownloadFinished(Err("network".to_string())));
        assert_eq!(
            events,
            vec![Event::DownloadFailed {
                pair: "en_ja".to_string(),
                message: "network".to_string()
            }]
        );
    }

    #[test]
    fn second_download_is_rejected_while_running() {
        let mut f = fixture();
        f.controller
            .handle(Command::StartDownload("en_ja".to_string()));

        let events = f
            .controller
            .handle(Command::StartDownload("ja_en".to_string()));
        assert_eq!(events.len(), 1);
        assert!(is_error_containing(&events[0], "already"));
    }

    #[test]
    fn invalid_pair_is_rejected_and_state_stays_idle() {
        let mut f = fixture();
        let events = f
            .controller
            .handle(Command::StartDownload("enja".to_string()));
        assert_eq!(events.len(), 1);
        assert!(is_error_containing(&events[0], "invalid"));
        assert_eq!(f.controller.download_state(), &DownloadState::Idle);
    }

    #[test]
    fn finishing_without_a_running_download_is_an_error() {
        let mut f = fixture();
        let events = f.controller.handle(Command::DownloadFinished(Ok(())));
        assert_eq!(events.len(), 1);
        assert!(is_error_containing(&events[0], "no download"));
    }

    #[test]
    fn missing_packages_follow_the_language_settings() {
        let f = fixture();
        assert_eq!(
            f.controller.missing_packages(&[]),
            vec!["en_ja".to_string()]
        );
        assert!(f
            .controller
            .missing_packages(&["en_ja".to_string()])
            .is_empty());
    }

    #[test]
    fn note_clipboard_written_via_command_prevents_translation() {
        let mut f = fixture();
        f.controller
            .handle(Command::NoteClipboardWritten("written text".to_string()));
        f.set_clipboard("written text");
        let events = f.controller.handle(Command::ClipboardTick);
        assert!(events.is_empty());
    }
}
