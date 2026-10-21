use crate::error::{AppError, Result};
use std::fmt;

/// ホットキーの修飾キー。宣言順が、表示と比較で使う正規化順になる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Modifier {
    Ctrl,
    Alt,
    Shift,
    Super,
}

impl Modifier {
    fn parse(token: &str) -> Option<Self> {
        match token.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => Some(Self::Ctrl),
            "alt" | "option" => Some(Self::Alt),
            "shift" => Some(Self::Shift),
            "super" | "win" | "windows" | "cmd" | "command" | "meta" => Some(Self::Super),
            _ => None,
        }
    }

    /// 正規化した表示名
    pub fn label(self) -> &'static str {
        match self {
            Self::Ctrl => "Ctrl",
            Self::Alt => "Alt",
            Self::Shift => "Shift",
            Self::Super => "Super",
        }
    }
}

/// 解析済みのホットキー。修飾キーは重複なし・正規化順で保持する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hotkey {
    modifiers: Vec<Modifier>,
    key: String,
}

impl Hotkey {
    /// `Ctrl+Alt+T` のような文字列を解析する。
    /// 修飾キーを 1 つ以上と、キーをちょうど 1 つ含む必要がある。
    /// 大文字・小文字は区別しない。
    pub fn parse(input: &str) -> Result<Self> {
        let mut modifiers: Vec<Modifier> = Vec::new();
        let mut key: Option<String> = None;

        for raw in input.split('+') {
            let token = raw.trim();
            if token.is_empty() {
                return Err(invalid(input, "empty element"));
            }

            if let Some(modifier) = Modifier::parse(token) {
                if modifiers.contains(&modifier) {
                    return Err(invalid(input, "duplicated modifier"));
                }
                modifiers.push(modifier);
            } else if key.is_none() {
                match normalize_key(token) {
                    Some(normalized) => key = Some(normalized),
                    None => return Err(invalid(input, "unsupported key")),
                }
            } else {
                return Err(invalid(input, "more than one key"));
            }
        }

        let key = key.ok_or_else(|| invalid(input, "no key"))?;
        if modifiers.is_empty() {
            return Err(invalid(input, "at least one modifier is required"));
        }
        modifiers.sort();

        Ok(Self { modifiers, key })
    }

    pub fn modifiers(&self) -> &[Modifier] {
        &self.modifiers
    }

    pub fn key(&self) -> &str {
        &self.key
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for modifier in &self.modifiers {
            write!(f, "{}+", modifier.label())?;
        }
        write!(f, "{}", self.key)
    }
}

fn invalid(input: &str, reason: &str) -> AppError {
    AppError::InvalidHotkey(format!("{input:?}: {reason}"))
}

/// キー名を正規化する。英数字 1 文字、F1〜F24、Space / Enter / Tab / Escape に対応する。
fn normalize_key(token: &str) -> Option<String> {
    let mut chars = token.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return if c.is_ascii_alphanumeric() {
            Some(c.to_ascii_uppercase().to_string())
        } else {
            None
        };
    }

    if let Some(rest) = token.strip_prefix(['F', 'f']) {
        if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
            let number: u32 = rest.parse().ok()?;
            return if (1..=24).contains(&number) {
                Some(format!("F{number}"))
            } else {
                None
            };
        }
    }

    match token.to_ascii_lowercase().as_str() {
        "space" => Some("Space".to_string()),
        "enter" | "return" => Some("Enter".to_string()),
        "tab" => Some("Tab".to_string()),
        "esc" | "escape" => Some("Escape".to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_normalizes() {
        let hotkey = Hotkey::parse("ctrl+alt+t").unwrap();
        assert_eq!(hotkey.to_string(), "Ctrl+Alt+T");
        assert_eq!(hotkey.modifiers(), &[Modifier::Ctrl, Modifier::Alt]);
        assert_eq!(hotkey.key(), "T");
    }

    #[test]
    fn modifier_order_does_not_matter() {
        let a = Hotkey::parse("Alt+Ctrl+T").unwrap();
        let b = Hotkey::parse("Ctrl+Alt+T").unwrap();
        assert_eq!(a, b);
        assert_eq!(a.to_string(), "Ctrl+Alt+T");
    }

    #[test]
    fn key_can_come_first_and_spaces_are_ignored() {
        let hotkey = Hotkey::parse(" t + Shift ").unwrap();
        assert_eq!(hotkey.to_string(), "Shift+T");
    }

    #[test]
    fn accepts_aliases_for_modifiers() {
        assert_eq!(
            Hotkey::parse("Control+Option+1").unwrap().to_string(),
            "Ctrl+Alt+1"
        );
        assert_eq!(
            Hotkey::parse("Win+Space").unwrap().to_string(),
            "Super+Space"
        );
        assert_eq!(
            Hotkey::parse("Cmd+Return").unwrap().to_string(),
            "Super+Enter"
        );
    }

    #[test]
    fn accepts_function_keys_within_range() {
        assert_eq!(Hotkey::parse("Ctrl+f1").unwrap().to_string(), "Ctrl+F1");
        assert_eq!(Hotkey::parse("Ctrl+F24").unwrap().to_string(), "Ctrl+F24");
        assert!(Hotkey::parse("Ctrl+F25").is_err());
        assert!(Hotkey::parse("Ctrl+F0").is_err());
    }

    #[test]
    fn single_letter_f_is_a_normal_key() {
        assert_eq!(Hotkey::parse("Ctrl+f").unwrap().to_string(), "Ctrl+F");
    }

    #[test]
    fn rejects_invalid_input() {
        // 修飾キーがない
        assert!(Hotkey::parse("T").is_err());
        // キーがない
        assert!(Hotkey::parse("Ctrl").is_err());
        assert!(Hotkey::parse("Ctrl+Alt").is_err());
        // キーが複数ある
        assert!(Hotkey::parse("Ctrl+A+B").is_err());
        // 修飾キーの重複
        assert!(Hotkey::parse("Ctrl+Control+A").is_err());
        // 空の要素
        assert!(Hotkey::parse("").is_err());
        assert!(Hotkey::parse("Ctrl+").is_err());
        assert!(Hotkey::parse("Ctrl++A").is_err());
        // 非対応のキー
        assert!(Hotkey::parse("Ctrl+日").is_err());
        assert!(Hotkey::parse("Ctrl+Foo").is_err());
    }

    #[test]
    fn error_is_invalid_hotkey_variant() {
        match Hotkey::parse("T") {
            Err(AppError::InvalidHotkey(message)) => assert!(message.contains("modifier")),
            other => panic!("unexpected result: {other:?}"),
        }
    }
}
