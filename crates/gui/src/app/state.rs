//! 画面に依存しない状態型（タブ、状態表示、翻訳リクエストの追跡）。

/// 画面タブ
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Translate,
    Models,
    Settings,
}

/// アプリケーションの状態表示種別（ADR-006 / Issue #61）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppStatus {
    /// 通常・情報メッセージ（準備完了、コピー完了など）
    Info(String),
    /// 処理中メッセージ（翻訳中、ダウンロード中など。スピナーを表示）
    Busy(String),
    /// エラー・警告メッセージ（エラー色で表示）
    Error(String),
}

impl AppStatus {
    #[allow(dead_code)]
    pub fn message(&self) -> &str {
        match self {
            AppStatus::Info(msg) | AppStatus::Busy(msg) | AppStatus::Error(msg) => msg,
        }
    }

    #[allow(dead_code)]
    pub fn is_busy(&self) -> bool {
        matches!(self, AppStatus::Busy(_))
    }
}

/// 自動翻訳のリクエスト管理と古い結果の破棄（Issue #60 / ADR-006）
#[derive(Debug, Clone, Default)]
pub struct RequestTracker {
    /// 発行済みの最新リクエストID
    next_id: u64,
    /// 現在有効な最新のリクエストID（これより古い結果は破棄する）
    active_id: u64,
    /// 最後にリクエストしたテキストの内容
    active_text: String,
    /// 処理待ちのリクエストキュー: (id, text, is_auto)
    pending_queue: std::collections::VecDeque<(u64, String, bool)>,
}

impl RequestTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// 新しいリクエストを発行し、そのリクエストIDを返す。
    pub fn issue_request(&mut self, text: String, is_auto: bool) -> u64 {
        self.next_id += 1;
        self.active_id = self.next_id;
        self.active_text = text.clone();
        self.pending_queue
            .push_back((self.active_id, text, is_auto));
        self.active_id
    }

    /// 入力が変更されたため、現在進行中のリクエストを無効化する
    pub fn invalidate(&mut self) {
        self.next_id += 1;
        self.active_id = self.next_id;
        self.active_text.clear();
    }

    /// 翻訳結果が返ってきたとき、キューの先頭リクエストを取り出して採用可否と is_auto を判定する
    pub fn on_translated(&mut self, source: &str) -> (bool, bool) {
        if let Some((id, text, is_auto)) = self.pending_queue.pop_front() {
            let accept = id == self.active_id && text.trim() == source.trim();
            (accept, is_auto)
        } else {
            // クリップボード監視等、キュー外からの翻訳結果
            let accept = self.active_text.trim() == source.trim();
            (accept, false)
        }
    }

    #[allow(dead_code)]
    pub fn active_text(&self) -> &str {
        &self.active_text
    }

    #[allow(dead_code)]
    pub fn active_id(&self) -> u64 {
        self.active_id
    }

    #[allow(dead_code)]
    pub fn pending_count(&self) -> usize {
        self.pending_queue.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_status_properties() {
        let info = AppStatus::Info("準備完了".to_string());
        assert_eq!(info.message(), "準備完了");
        assert!(!info.is_busy());

        let busy = AppStatus::Busy("翻訳中…".to_string());
        assert_eq!(busy.message(), "翻訳中…");
        assert!(busy.is_busy());

        let error = AppStatus::Error("エラー発生".to_string());
        assert_eq!(error.message(), "エラー発生");
        assert!(!error.is_busy());
    }

    #[test]
    fn request_tracker_basic_flow() {
        let mut tracker = RequestTracker::new();
        let id1 = tracker.issue_request("Hello".to_string(), true);
        assert_eq!(tracker.active_id(), id1);
        assert_eq!(tracker.pending_count(), 1);
        assert_eq!(tracker.active_text(), "Hello");

        let (accept, is_auto) = tracker.on_translated("Hello");
        assert!(accept);
        assert!(is_auto);
        assert_eq!(tracker.pending_count(), 0);
    }

    #[test]
    fn request_tracker_invalidate_discards_previous() {
        let mut tracker = RequestTracker::new();
        let _id1 = tracker.issue_request("Hello".to_string(), true);
        tracker.invalidate();

        let (accept, _is_auto) = tracker.on_translated("Hello");
        assert!(!accept);
    }

    #[test]
    fn request_tracker_superseded_request_discarded() {
        let mut tracker = RequestTracker::new();
        let _id1 = tracker.issue_request("Hello".to_string(), true);
        let id2 = tracker.issue_request("Hello World".to_string(), true);
        assert_eq!(tracker.active_id(), id2);
        assert_eq!(tracker.pending_count(), 2);

        // 1件目の結果到着 -> 古いので破棄
        let (accept1, _) = tracker.on_translated("Hello");
        assert!(!accept1);

        // 2件目の結果到着 -> 最新なので採用
        let (accept2, is_auto) = tracker.on_translated("Hello World");
        assert!(accept2);
        assert!(is_auto);
    }

    #[test]
    fn request_tracker_text_mismatch_discarded() {
        let mut tracker = RequestTracker::new();
        let _id = tracker.issue_request("Hello".to_string(), false);

        // テキスト内容が異なれば破棄
        let (accept, _) = tracker.on_translated("Different text");
        assert!(!accept);
    }
}
