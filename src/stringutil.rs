#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WordCounter {
    Full,
    Letter,
    Fast,
}

impl WordCounter {
    pub fn select(text: &str) -> Self {
        if text.chars().any(is_cjk) {
            Self::Full
        } else if text.chars().any(is_hangul) {
            Self::Letter
        } else {
            Self::Fast
        }
    }

    pub fn count(self, text: &str) -> usize {
        let words = text
            .split(is_ascii_space)
            .filter(|word| {
                word.chars().any(|character| {
                    is_letter(character) || (self != Self::Fast && is_hangul(character))
                })
            })
            .count();
        if self == Self::Full {
            let ideographs = text.chars().filter(|&character| is_cjk(character)).count();
            words + (ideographs as f64 * 0.55).ceil() as usize
        } else {
            words
        }
    }
}

pub fn is_ascii_space(character: char) -> bool {
    matches!(character, '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ')
}

fn is_letter(character: char) -> bool {
    matches!(character, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '\u{c0}'..='\u{1fff}')
}

fn is_hangul(character: char) -> bool {
    matches!(character, '\u{ac00}'..='\u{d7af}')
}

fn is_cjk(character: char) -> bool {
    matches!(character, '\u{3040}'..='\u{a4cf}')
}

#[cfg(test)]
mod tests {
    use super::WordCounter;

    #[test]
    fn go_word_counters_preserve_ascii_delimiters_and_ranges() {
        for counter in [WordCounter::Fast, WordCounter::Letter, WordCounter::Full] {
            for (input, expected) in [
                ("", 0),
                ("  -@# ';]", 0),
                ("b'fore", 1),
                (" _word.under_score_ ", 1),
                (" \ttwo\nwords", 2),
                ("one\u{a0}two", 1),
                ("one\u{b}two\u{c}three", 3),
                ("\u{bf}", 0),
                ("\u{c0}", 1),
                ("\u{1fff}", 1),
                ("\u{2000}", 0),
            ] {
                assert_eq!(counter.count(input), expected, "{counter:?}: {input:?}");
            }
        }
        assert_eq!(WordCounter::Full.count("word\u{5b57}"), 2);
        assert_eq!(
            WordCounter::Full.count("\u{4ecf}\u{4eee}\u{99c5}\u{8fba}"),
            3
        );
        assert_eq!(
            WordCounter::Letter.count("\u{d55c}\u{ad6d}\u{c5b4} \u{b2e8}\u{c5b4}"),
            2
        );
        assert_eq!(
            WordCounter::Fast.count("\u{d55c}\u{ad6d}\u{c5b4} \u{b2e8}\u{c5b4}"),
            0
        );
    }
}
