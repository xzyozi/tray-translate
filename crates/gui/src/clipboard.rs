use tray_translate_app::ClipboardSource;

/// OS のクリップボードを読む `ClipboardSource` の実装（テキストのみ）。
/// 読み取りに失敗した場合や、テキスト以外が入っている場合は `None` を返す。
#[derive(Default)]
pub struct ArboardClipboard {
    inner: Option<arboard::Clipboard>,
}

impl ClipboardSource for ArboardClipboard {
    fn read_text(&mut self) -> Option<String> {
        if self.inner.is_none() {
            self.inner = arboard::Clipboard::new().ok();
        }
        self.inner.as_mut()?.get_text().ok()
    }
}
