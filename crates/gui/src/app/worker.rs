//! ワーカースレッド向けの tick 集約。キューに溜まった tick を 1 回分に間引く。

use std::sync::mpsc::Receiver;
use tray_translate_app::Command;

/// キューに溜まった tick を 1 回分に集約した結果
pub(super) struct TickBatch {
    /// クリップボード確認を行うか
    pub(super) clipboard: bool,
    /// アイドル解放の確認を行うか
    pub(super) idle: bool,
    /// tick の後ろで最初に見つかった、tick 以外の操作（処理順を保つため、tick の後に実行する）
    pub(super) next: Option<Command>,
}

/// `first`（tick）に続く tick をキューから取り出して集約する。
/// tick 以外の操作に当たったら、そこで止めて `next` に返す。
pub(super) fn coalesce_ticks(first: &Command, rx: &Receiver<Command>) -> TickBatch {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

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
        tx.send(Command::TranslateText("hello".to_string()))
            .unwrap();
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
