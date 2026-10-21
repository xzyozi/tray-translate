/// クリップボードの読み取り元。実体は OS ごとの実装で、テストでは偽物に差し替える。
pub trait ClipboardSource {
    /// 現在のテキストを返す。空、またはテキスト以外の場合は `None`。
    fn read_text(&mut self) -> Option<String>;
}

/// クリップボードの変化を検出し、翻訳対象の新しいテキストだけを取り出す。
///
/// - 前回と同じ内容は無視する。
/// - 文字数が範囲外のテキストは無視する（前後の空白を除いて数える）。
/// - 起動時にすでに入っていた内容は `prime` で既読にできる。
/// - アプリ自身が書き込んだ内容は `mark_written` で既読にできる。
pub struct ClipboardWatcher {
    last_seen: Option<String>,
    min_chars: usize,
    max_chars: usize,
}

impl ClipboardWatcher {
    pub fn new(min_chars: usize, max_chars: usize) -> Self {
        Self {
            last_seen: None,
            min_chars,
            max_chars,
        }
    }

    /// 現在の内容を既読にする（起動直後の呼び出し用）
    pub fn prime(&mut self, source: &mut dyn ClipboardSource) {
        self.last_seen = source.read_text();
    }

    /// アプリが書き込んだテキストを既読にする（自分の出力を再翻訳しないため）
    pub fn mark_written(&mut self, text: &str) {
        self.last_seen = Some(text.to_string());
    }

    /// クリップボードを確認し、翻訳対象の新しいテキストがあれば返す
    pub fn poll(&mut self, source: &mut dyn ClipboardSource) -> Option<String> {
        let Some(text) = source.read_text() else {
            // 空になった場合は既読の記録を消す（同じ文をもう一度コピーしたときに検出できる）
            self.last_seen = None;
            return None;
        };

        if self.last_seen.as_deref() == Some(text.as_str()) {
            return None;
        }
        self.last_seen = Some(text.clone());

        let len = text.trim().chars().count();
        if len < self.min_chars || len > self.max_chars {
            return None;
        }
        Some(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 固定の内容を返す偽のクリップボード
    struct FakeClipboard {
        text: Option<String>,
    }

    impl ClipboardSource for FakeClipboard {
        fn read_text(&mut self) -> Option<String> {
            self.text.clone()
        }
    }

    fn clip(text: Option<&str>) -> FakeClipboard {
        FakeClipboard {
            text: text.map(|t| t.to_string()),
        }
    }

    /// クリップボードの内容を指定して、1 回確認する
    fn poll_with(watcher: &mut ClipboardWatcher, text: Option<&str>) -> Option<String> {
        watcher.poll(&mut clip(text))
    }

    #[test]
    fn returns_new_text_once() {
        let mut watcher = ClipboardWatcher::new(1, 100);
        assert_eq!(
            poll_with(&mut watcher, Some("hello")),
            Some("hello".to_string())
        );
        assert_eq!(poll_with(&mut watcher, Some("hello")), None);
        assert_eq!(
            poll_with(&mut watcher, Some("world")),
            Some("world".to_string())
        );
    }

    #[test]
    fn same_text_after_a_different_text_is_detected_again() {
        let mut watcher = ClipboardWatcher::new(1, 100);
        assert_eq!(poll_with(&mut watcher, Some("a1")), Some("a1".to_string()));
        assert_eq!(poll_with(&mut watcher, Some("b2")), Some("b2".to_string()));
        assert_eq!(poll_with(&mut watcher, Some("a1")), Some("a1".to_string()));
    }

    #[test]
    fn same_text_after_clipboard_was_emptied_is_detected_again() {
        let mut watcher = ClipboardWatcher::new(1, 100);
        assert_eq!(
            poll_with(&mut watcher, Some("hello")),
            Some("hello".to_string())
        );
        assert_eq!(poll_with(&mut watcher, None), None);
        assert_eq!(
            poll_with(&mut watcher, Some("hello")),
            Some("hello".to_string())
        );
    }

    #[test]
    fn prime_marks_existing_content_as_seen() {
        let mut watcher = ClipboardWatcher::new(1, 100);
        watcher.prime(&mut clip(Some("old")));
        assert_eq!(poll_with(&mut watcher, Some("old")), None);
        assert_eq!(
            poll_with(&mut watcher, Some("new")),
            Some("new".to_string())
        );
    }

    #[test]
    fn own_output_is_not_translated_again() {
        let mut watcher = ClipboardWatcher::new(1, 100);
        watcher.mark_written("translated");
        assert_eq!(poll_with(&mut watcher, Some("translated")), None);
        assert_eq!(
            poll_with(&mut watcher, Some("next")),
            Some("next".to_string())
        );
    }

    #[test]
    fn text_outside_the_length_range_is_ignored() {
        let mut watcher = ClipboardWatcher::new(3, 5);
        assert_eq!(poll_with(&mut watcher, Some("ab")), None);
        assert_eq!(poll_with(&mut watcher, Some("abcdef")), None);
        assert_eq!(poll_with(&mut watcher, Some("   x   ")), None);
        assert_eq!(
            poll_with(&mut watcher, Some("abc")),
            Some("abc".to_string())
        );
        assert_eq!(
            poll_with(&mut watcher, Some("abcde")),
            Some("abcde".to_string())
        );
    }

    #[test]
    fn length_is_counted_in_characters() {
        let mut watcher = ClipboardWatcher::new(1, 5);
        let output = poll_with(&mut watcher, Some("あいうえお"));
        assert_eq!(output, Some("あいうえお".to_string()));
    }

    #[test]
    fn empty_clipboard_returns_none() {
        let mut watcher = ClipboardWatcher::new(1, 100);
        assert_eq!(poll_with(&mut watcher, None), None);
        assert_eq!(poll_with(&mut watcher, None), None);
    }

    #[test]
    fn out_of_range_text_is_not_reported_again_either() {
        let mut watcher = ClipboardWatcher::new(3, 100);
        assert_eq!(poll_with(&mut watcher, Some("ab")), None);
        assert_eq!(poll_with(&mut watcher, Some("ab")), None);
    }
}
