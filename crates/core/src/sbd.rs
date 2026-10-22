/// 単純かつ高速なルールベース文分割器（Sentence Boundary Detector）
pub struct RuleBasedSentenceSplitter;

impl RuleBasedSentenceSplitter {
    /// テキストを文のリストに分割する。
    /// 英語のピリオド・感嘆符・疑問符、全角の句点・感嘆符・疑問符（。！？）および改行で分割する。
    pub fn split(text: &str) -> Vec<String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }

        let mut sentences = Vec::new();
        let mut current = String::new();
        let chars: Vec<char> = trimmed.chars().collect();
        let len = chars.len();
        let mut i = 0;

        while i < len {
            let c = chars[i];
            current.push(c);

            let is_potential_boundary = match c {
                '。' | '！' | '？' => true,
                '\n' => true,
                '!' | '?' => {
                    // 次の文字が閉じ記号、空白、または終端であれば境界
                    if i + 1 < len {
                        chars[i + 1].is_whitespace() || is_closing_bracket(chars[i + 1])
                    } else {
                        true
                    }
                }
                '.' => {
                    // ピリオドの場合：略語や小数点、拡張子などを除外
                    if is_abbreviation_or_number(&current, &chars, i, len) {
                        false
                    } else if i + 1 < len {
                        chars[i + 1].is_whitespace() || is_closing_bracket(chars[i + 1])
                    } else {
                        true
                    }
                }
                _ => false,
            };

            if is_potential_boundary {
                // 境界文字の直後に連続する句読点・感嘆符・疑問符（例: ！？, ..., !!）も同じ文に含める
                while i + 1 < len && is_punctuation(chars[i + 1]) {
                    i += 1;
                    current.push(chars[i]);
                }

                // 境界文字の直後に閉じ引用符や閉じ括弧が続く場合は、それらも現在の文に含める
                while i + 1 < len && is_closing_bracket(chars[i + 1]) {
                    i += 1;
                    current.push(chars[i]);
                }

                let s = current.trim();
                if !s.is_empty() {
                    sentences.push(s.to_string());
                }
                current.clear();
            }

            i += 1;
        }

        let s = current.trim();
        if !s.is_empty() {
            sentences.push(s.to_string());
        }

        sentences
    }
}

fn is_punctuation(c: char) -> bool {
    matches!(c, '。' | '！' | '？' | '.' | '!' | '?' | '…')
}

fn is_closing_bracket(c: char) -> bool {
    matches!(
        c,
        '"' | '\'' | '”' | '’' | '»' | ')' | ']' | '}' | '）' | '」' | '』' | '】' | '〉' | '》'
    )
}

fn is_abbreviation_or_number(current: &str, chars: &[char], i: usize, len: usize) -> bool {
    // 1. 次の文字が数字の場合（例: 3.14）
    if i + 1 < len && chars[i + 1].is_ascii_digit() {
        return true;
    }

    // 2. 現在のトークン（最後の単語）を取り出して略語判定
    let last_word = current
        .split_whitespace()
        .last()
        .unwrap_or("")
        .trim_matches(|c: char| !c.is_alphabetic() && c != '.');

    let lower = last_word.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "mr."
            | "mrs."
            | "ms."
            | "dr."
            | "prof."
            | "sr."
            | "jr."
            | "vs."
            | "etc."
            | "e.g."
            | "i.e."
            | "u.s."
            | "u.k."
            | "st."
            | "no."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_english() {
        let text = "Hello world. How are you? I am fine!";
        let sentences = RuleBasedSentenceSplitter::split(text);
        assert_eq!(
            sentences,
            vec!["Hello world.", "How are you?", "I am fine!"]
        );
    }

    #[test]
    fn test_split_japanese() {
        let text = "こんにちは。今日はいい天気ですね。散歩に行きましょう。";
        let sentences = RuleBasedSentenceSplitter::split(text);
        assert_eq!(
            sentences,
            vec![
                "こんにちは。",
                "今日はいい天気ですね。",
                "散歩に行きましょう。"
            ]
        );
    }

    #[test]
    fn test_split_japanese_fullwidth_punctuation() {
        let text = "マジで！？本当に！すごいね。";
        let sentences = RuleBasedSentenceSplitter::split(text);
        assert_eq!(sentences, vec!["マジで！？", "本当に！", "すごいね。"]);
    }

    #[test]
    fn test_split_with_quotes() {
        let text = "He said, \"Hello!\" Then he walked away.";
        let sentences = RuleBasedSentenceSplitter::split(text);
        assert_eq!(
            sentences,
            vec!["He said, \"Hello!\"", "Then he walked away."]
        );
    }

    #[test]
    fn test_split_with_abbreviations() {
        let text = "Dr. Smith and Mr. Brown visited the U.S. yesterday. It was great.";
        let sentences = RuleBasedSentenceSplitter::split(text);
        assert_eq!(
            sentences,
            vec![
                "Dr. Smith and Mr. Brown visited the U.S. yesterday.",
                "It was great."
            ]
        );
    }

    #[test]
    fn test_split_with_numbers() {
        let text = "The value is 3.14. Next item is 2.5.";
        let sentences = RuleBasedSentenceSplitter::split(text);
        assert_eq!(sentences, vec!["The value is 3.14.", "Next item is 2.5."]);
    }

    #[test]
    fn test_split_with_newlines() {
        let text = "Line one.\nLine two.";
        let sentences = RuleBasedSentenceSplitter::split(text);
        assert_eq!(sentences, vec!["Line one.", "Line two."]);
    }
}
