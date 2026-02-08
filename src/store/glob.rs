/// Simple glob pattern matching supporting * (any sequence) and ? (single char)
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    glob_match_recursive(&pattern, &text, 0, 0)
}

fn glob_match_recursive(pattern: &[char], text: &[char], pi: usize, ti: usize) -> bool {
    // Base case: pattern exhausted
    if pi == pattern.len() {
        return ti == text.len();
    }

    match pattern[pi] {
        '*' => {
            // Try matching * with 0 or more characters
            for i in ti..=text.len() {
                if glob_match_recursive(pattern, text, pi + 1, i) {
                    return true;
                }
            }
            false
        }
        '?' => {
            // Match exactly one character
            if ti < text.len() {
                glob_match_recursive(pattern, text, pi + 1, ti + 1)
            } else {
                false
            }
        }
        c => {
            // Match literal character
            if ti < text.len() && text[ti] == c {
                glob_match_recursive(pattern, text, pi + 1, ti + 1)
            } else {
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_match_star() {
        assert!(glob_match("*", "anything"));
        assert!(glob_match("*", ""));
        assert!(glob_match("foo*", "foobar"));
        assert!(glob_match("foo*", "foo"));
        assert!(glob_match("*bar", "foobar"));
        assert!(glob_match("*bar", "bar"));
        assert!(glob_match("*oba*", "foobar"));
        assert!(!glob_match("foo*", "bar"));
        assert!(!glob_match("*foo", "foobar"));
    }

    #[test]
    fn test_glob_match_question() {
        assert!(glob_match("?", "a"));
        assert!(!glob_match("?", ""));
        assert!(!glob_match("?", "ab"));
        assert!(glob_match("fo?", "foo"));
        assert!(glob_match("f??", "foo"));
        assert!(!glob_match("f?", "foo"));
        assert!(glob_match("???", "abc"));
    }

    #[test]
    fn test_glob_match_literal() {
        assert!(glob_match("exact", "exact"));
        assert!(!glob_match("exact", "exactx"));
        assert!(!glob_match("exactx", "exact"));
        assert!(!glob_match("foo", "bar"));
    }

    #[test]
    fn test_glob_match_combined() {
        assert!(glob_match("user:*:name", "user:123:name"));
        assert!(glob_match("user:*:name", "user::name"));
        assert!(!glob_match("user:*:name", "user:123:age"));
        assert!(glob_match("key?_*", "key1_value"));
        assert!(glob_match("key?_*", "key1_"));
        assert!(!glob_match("key?_*", "key12_value"));
        assert!(glob_match("*?*", "a"));
        assert!(!glob_match("*?*", ""));
    }
}
