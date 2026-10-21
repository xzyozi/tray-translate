use crate::config::Settings;
use crate::error::{AppError, Result};
use std::time::{Duration, Instant};
use tray_translate_core::TranslationRouter;

/// 翻訳バックエンド。実体は `TranslationRouter` で、テストでは偽物に差し替える。
pub trait Translate {
    fn translate(&mut self, from: &str, to: &str, text: &str) -> Result<String>;
    /// ロード済みのモデルをすべて解放する
    fn unload_all(&mut self);
    /// 現在ロードされているモデル数
    fn loaded_count(&self) -> usize;
}

impl Translate for TranslationRouter {
    fn translate(&mut self, from: &str, to: &str, text: &str) -> Result<String> {
        Ok(TranslationRouter::translate(self, from, to, text)?)
    }

    fn unload_all(&mut self) {
        TranslationRouter::unload_all(self);
    }

    fn loaded_count(&self) -> usize {
        TranslationRouter::loaded_count(self)
    }
}

/// 経過時間の取得元。テストでは時刻を自由に進められる偽物に差し替える。
pub trait Clock {
    /// 基準時点からの経過時間（単調増加）
    fn now(&self) -> Duration;
}

/// 実時間の時計
pub struct SystemClock {
    start: Instant,
}

impl SystemClock {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Duration {
        self.start.elapsed()
    }
}

/// 翻訳サービスの設定
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceConfig {
    pub source_lang: String,
    pub target_lang: String,
    pub min_chars: usize,
    pub max_chars: usize,
    /// `None` の場合は、モデルを自動で解放しない
    pub idle_unload: Option<Duration>,
}

impl From<&Settings> for ServiceConfig {
    fn from(settings: &Settings) -> Self {
        let idle_unload = if settings.idle_unload_secs == 0 {
            None
        } else {
            Some(Duration::from_secs(settings.idle_unload_secs))
        };
        Self {
            source_lang: settings.source_lang.clone(),
            target_lang: settings.target_lang.clone(),
            min_chars: settings.min_chars,
            max_chars: settings.max_chars,
            idle_unload,
        }
    }
}

/// 常駐アプリの翻訳サービス。
/// 入力文字数の検査と、一定時間使われなかったモデルの解放を担う。
pub struct TranslationService<T: Translate, C: Clock> {
    backend: T,
    clock: C,
    config: ServiceConfig,
    last_used: Option<Duration>,
}

impl<T: Translate, C: Clock> TranslationService<T, C> {
    pub fn new(backend: T, clock: C, config: ServiceConfig) -> Self {
        Self {
            backend,
            clock,
            config,
            last_used: None,
        }
    }

    pub fn config(&self) -> &ServiceConfig {
        &self.config
    }

    pub fn set_config(&mut self, config: ServiceConfig) {
        self.config = config;
    }

    pub fn backend(&self) -> &T {
        &self.backend
    }

    /// テキストを翻訳する。
    ///
    /// - 前後の空白を除いた文字数が `min_chars` 未満なら、翻訳せず `Ok(None)` を返す。
    /// - `max_chars` を超える場合は `TextTooLong` エラーを返す。
    /// - 文字数は Unicode のスカラー値の数で数える（バイト数ではない）。
    pub fn translate(&mut self, text: &str) -> Result<Option<String>> {
        let trimmed = text.trim();
        let len = trimmed.chars().count();

        if len < self.config.min_chars {
            return Ok(None);
        }
        if len > self.config.max_chars {
            return Err(AppError::TextTooLong {
                len,
                max: self.config.max_chars,
            });
        }

        let output = self.backend.translate(
            &self.config.source_lang,
            &self.config.target_lang,
            trimmed,
        )?;
        self.last_used = Some(self.clock.now());
        Ok(Some(output))
    }

    /// 最後の翻訳から `idle_unload` 以上経過していて、モデルがロード中なら解放する。
    /// 解放した場合は `true` を返す。定期的に呼び出す想定。
    pub fn unload_if_idle(&mut self) -> bool {
        let Some(idle) = self.config.idle_unload else {
            return false;
        };
        let Some(last_used) = self.last_used else {
            return false;
        };
        if self.backend.loaded_count() == 0 {
            return false;
        }

        let elapsed = self.clock.now().saturating_sub(last_used);
        if elapsed >= idle {
            self.backend.unload_all();
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[derive(Default)]
    struct FakeBackend {
        loaded: usize,
        calls: Vec<String>,
        fail: bool,
    }

    impl Translate for FakeBackend {
        fn translate(&mut self, from: &str, to: &str, text: &str) -> Result<String> {
            if self.fail {
                return Err(AppError::InvalidSettings("backend failure".to_string()));
            }
            self.loaded = 1;
            self.calls.push(text.to_string());
            Ok(format!("[{from}->{to}] {text}"))
        }

        fn unload_all(&mut self) {
            self.loaded = 0;
        }

        fn loaded_count(&self) -> usize {
            self.loaded
        }
    }

    #[derive(Clone)]
    struct FakeClock(Rc<Cell<Duration>>);

    impl Clock for FakeClock {
        fn now(&self) -> Duration {
            self.0.get()
        }
    }

    fn config(idle_unload: Option<Duration>) -> ServiceConfig {
        ServiceConfig {
            source_lang: "en".to_string(),
            target_lang: "ja".to_string(),
            min_chars: 3,
            max_chars: 10,
            idle_unload,
        }
    }

    fn make_service(
        idle_unload: Option<Duration>,
    ) -> (TranslationService<FakeBackend, FakeClock>, Rc<Cell<Duration>>) {
        let time = Rc::new(Cell::new(Duration::ZERO));
        let clock = FakeClock(Rc::clone(&time));
        let service = TranslationService::new(FakeBackend::default(), clock, config(idle_unload));
        (service, time)
    }

    #[test]
    fn translates_trimmed_text() {
        let (mut service, _) = make_service(None);
        let output = service.translate("  hello  ").unwrap();
        assert_eq!(output, Some("[en->ja] hello".to_string()));
        assert_eq!(service.backend().calls, vec!["hello".to_string()]);
    }

    #[test]
    fn short_or_blank_text_is_skipped() {
        let (mut service, _) = make_service(None);
        assert_eq!(service.translate("ab").unwrap(), None);
        assert_eq!(service.translate("     ").unwrap(), None);
        assert_eq!(service.translate("").unwrap(), None);
        assert!(service.backend().calls.is_empty());
    }

    #[test]
    fn too_long_text_is_rejected() {
        let (mut service, _) = make_service(None);
        let result = service.translate("12345678901");
        assert!(matches!(result, Err(AppError::TextTooLong { len: 11, max: 10 })));
        assert!(service.backend().calls.is_empty());
    }

    #[test]
    fn length_is_counted_in_characters_not_bytes() {
        let (mut service, _) = make_service(None);
        // 10 文字（30 バイト）は上限ちょうどなので翻訳できる
        let text = "あいうえおかきくけこ";
        assert!(service.translate(text).unwrap().is_some());
        // 11 文字は上限超過
        let result = service.translate("あいうえおかきくけこさ");
        assert!(matches!(result, Err(AppError::TextTooLong { .. })));
    }

    #[test]
    fn backend_failure_is_propagated() {
        let (mut service, _) = make_service(Some(Duration::from_secs(10)));
        service.backend.fail = true;
        assert!(service.translate("hello").is_err());
        // 失敗した翻訳は「使用した」ことにならない
        assert!(!service.unload_if_idle());
    }

    #[test]
    fn unloads_only_after_idle_period() {
        let idle = Duration::from_secs(10);
        let (mut service, time) = make_service(Some(idle));

        service.translate("hello").unwrap();
        assert_eq!(service.backend().loaded_count(), 1);

        time.set(Duration::from_secs(9));
        assert!(!service.unload_if_idle());
        assert_eq!(service.backend().loaded_count(), 1);

        time.set(Duration::from_secs(10));
        assert!(service.unload_if_idle());
        assert_eq!(service.backend().loaded_count(), 0);

        // 解放済みなら、何度呼んでも何もしない
        time.set(Duration::from_secs(100));
        assert!(!service.unload_if_idle());
    }

    #[test]
    fn using_the_service_resets_the_idle_timer() {
        let idle = Duration::from_secs(10);
        let (mut service, time) = make_service(Some(idle));

        service.translate("hello").unwrap();
        time.set(Duration::from_secs(8));
        service.translate("world").unwrap();

        // 最初の翻訳から 12 秒だが、最後の翻訳からは 4 秒
        time.set(Duration::from_secs(12));
        assert!(!service.unload_if_idle());

        time.set(Duration::from_secs(18));
        assert!(service.unload_if_idle());
    }

    #[test]
    fn model_is_reloaded_on_next_translation() {
        let (mut service, time) = make_service(Some(Duration::from_secs(1)));

        service.translate("hello").unwrap();
        time.set(Duration::from_secs(5));
        assert!(service.unload_if_idle());
        assert_eq!(service.backend().loaded_count(), 0);

        service.translate("again").unwrap();
        assert_eq!(service.backend().loaded_count(), 1);
    }

    #[test]
    fn never_unloads_when_idle_unload_is_disabled() {
        let (mut service, time) = make_service(None);
        service.translate("hello").unwrap();
        time.set(Duration::from_secs(100_000));
        assert!(!service.unload_if_idle());
        assert_eq!(service.backend().loaded_count(), 1);
    }

    #[test]
    fn does_nothing_before_first_translation() {
        let (mut service, time) = make_service(Some(Duration::from_secs(1)));
        time.set(Duration::from_secs(100));
        assert!(!service.unload_if_idle());
    }

    #[test]
    fn config_can_be_replaced() {
        let (mut service, _) = make_service(None);
        let mut new_config = config(None);
        new_config.target_lang = "es".to_string();
        service.set_config(new_config);

        let output = service.translate("hello").unwrap();
        assert_eq!(output, Some("[en->es] hello".to_string()));
    }

    #[test]
    fn service_config_is_built_from_settings() {
        let settings = Settings {
            idle_unload_secs: 0,
            ..Settings::default()
        };
        assert_eq!(ServiceConfig::from(&settings).idle_unload, None);

        let settings = Settings {
            idle_unload_secs: 300,
            ..Settings::default()
        };
        let config = ServiceConfig::from(&settings);
        assert_eq!(config.idle_unload, Some(Duration::from_secs(300)));
        assert_eq!(config.source_lang, settings.source_lang);
        assert_eq!(config.max_chars, settings.max_chars);
    }
}
