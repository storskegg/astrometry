//! Lenient matching of names against canonical labels, for the enums' parsers.

/// Lowercases `s` and strips spaces, tabs, underscores, hyphens and apostrophes, so
/// "North Node", "north_node" and "NorthNode" compare equal.
pub(crate) fn normalize(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| !matches!(c, ' ' | '\t' | '_' | '-' | '\''))
        .collect()
}

/// The position of `name` in `labels` after normalizing both.
pub(crate) fn index(labels: &[&str], name: &str) -> Option<usize> {
    let n = normalize(name);
    if n.is_empty() {
        return None;
    }
    labels.iter().position(|l| normalize(l) == n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_cases() {
        let tests = [
            ("North Node", "northnode"),
            ("north_node", "northnode"),
            ("NORTH-NODE", "northnode"),
            ("  Moon\t", "moon"),
            ("Caput Draconis'", "caputdraconis"),
            ("", ""),
            (" _-' ", ""),
            ("Ünïcode", "ünïcode"),
        ];
        for (input, want) in tests {
            assert_eq!(normalize(input), want, "normalize({input:?})");
        }
    }

    #[test]
    fn index_cases() {
        let labels = ["Sun", "Moon", "North Node"];
        let tests = [
            ("Sun", Some(0)),
            ("moon", Some(1)),
            ("north_node", Some(2)),
            ("NorthNode", Some(2)),
            ("Mars", None),
            ("", None),
            ("   ", None),
        ];
        for (name, want) in tests {
            assert_eq!(index(&labels, name), want, "index({name:?})");
        }
    }

    // An empty label must not match input that's empty once normalized.
    #[test]
    fn index_empty_label() {
        assert_eq!(index(&[""], "-"), None);
    }
}
