/// 単純かつ高速なルールベース文分割器（Sentence Boundary Detector）
pub struct RuleBasedSentenceSplitter;

impl RuleBasedSentenceSplitter {
    /// テキストを文のリストに分割する。
    /// 英語のピリオド・感嘆符・疑問符、日本語の句点（。）および改行で分割する。
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

            let is_boundary = match c {
                '。' => true,
                '\n' => true,
                '.' | '!' | '?' => {
                    // 次の文字が空白または終端である場合、文境界とみなす（略語や小数点の誤分割を抑制）
                    if i + 1 < len {
                        chars[i + 1].is_whitespace()
                    } else {
                        true
                    }
                }
                _ => false,
            };

            if is_boundary {
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
    fn test_split_with_newlines() {
        let text = "Line one.\nLine two.";
        let sentences = RuleBasedSentenceSplitter::split(text);
        assert_eq!(sentences, vec!["Line one.", "Line two."]);
    }
}
