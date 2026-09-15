use crate::urlutil::Url;
use regex::Regex;
use std::{collections::BTreeMap, sync::LazyLock};

const PLACEHOLDER: &str = "[*!]";
type Queries = BTreeMap<String, Vec<String>>;

fn bad_name(name: &str) -> bool {
    matches!(
        name,
        "baixar-gratis"
            | "category"
            | "content"
            | "day"
            | "date"
            | "definition"
            | "etiket"
            | "film-seyret"
            | "key"
            | "keys"
            | "keyword"
            | "label"
            | "news"
            | "q"
            | "query"
            | "rating"
            | "s"
            | "search"
            | "seasons"
            | "search_keyword"
            | "search_query"
            | "sortby"
            | "subscriptions"
            | "tag"
            | "tags"
            | "video"
            | "videos"
            | "w"
            | "wiki"
    )
}

fn expressions() -> &'static [Regex; 4] {
    static EXPRESSIONS: LazyLock<[Regex; 4]> = LazyLock::new(|| {
        [
            r"[0-9]+",
            r"(?i)(.s?html?)?$",
            r"(?i)([^/]*)/$",
            r"(?i)(?:/|(.html?))$",
        ]
        .map(|pattern| Regex::new(pattern).unwrap())
    });
    &EXPRESSIONS
}

fn path(url: &Url) -> String {
    String::from_utf8_lossy(&url.path).into_owned()
}

fn queries(url: &Url) -> Queries {
    let mut queries = Queries::new();
    if url.query.bytes().filter(|&byte| byte == b'&').count() >= 10_000 {
        return queries;
    }
    for pair in url.query.split('&') {
        if pair.is_empty() || pair.contains(';') {
            continue;
        }
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let (Some(key), Some(value)) = (
            crate::urlutil::unescape(&key.replace('+', " ")),
            crate::urlutil::unescape(&value.replace('+', " ")),
        ) else {
            continue;
        };
        queries
            .entry(String::from_utf8_lossy(&key).into_owned())
            .or_default()
            .push(String::from_utf8_lossy(&value).into_owned());
    }
    queries
}

fn query_escape(value: &str) -> String {
    const ESCAPED: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'_')
        .remove(b'.')
        .remove(b'~');
    percent_encoding::utf8_percent_encode(value, ESCAPED)
        .to_string()
        .replace("%20", "+")
}

fn encode_query(queries: &Queries) -> String {
    queries
        .iter()
        .flat_map(|(key, values)| {
            values
                .iter()
                .map(move |value| format!("{}={}", query_escape(key), query_escape(value)))
        })
        .collect::<Vec<_>>()
        .join("&")
}

#[derive(Clone, Debug)]
enum Kind {
    Query(Queries),
    Path {
        parameter: usize,
        start: usize,
        segment: usize,
        prefix: String,
        suffix: String,
    },
}

#[derive(Clone, Debug)]
pub(super) struct Pattern {
    pub value: String,
    pub number: i64,
    url: Url,
    path: String,
    kind: Kind,
}

impl Pattern {
    pub fn valid_for(&self, document: &Url) -> bool {
        self.valid_for_path(document, &path(document))
    }

    pub fn valid_for_path(&self, document: &Url, document_path: &str) -> bool {
        match &self.kind {
            Kind::Query(_) => {
                self.url.scheme == document.scheme
                    && self.url.host == document.host
                    && expressions()[3].replace_all(&self.path, "")
                        == expressions()[3].replace_all(document_path, "")
            }
            Kind::Path { parameter, .. } => {
                let components = document_path.split('/').collect::<Vec<_>>();
                let pattern = self.path.split('/').collect::<Vec<_>>();
                if components.len() > pattern.len() {
                    return false;
                }
                if components.len() == 1 && pattern.len() == 1 {
                    let prefix = components[0]
                        .bytes()
                        .zip(pattern[0].bytes())
                        .take_while(|(left, right)| left == right)
                        .count();
                    let mut suffix = 0;
                    let mut left = components[0].len() as isize - 1;
                    let mut right = pattern[0].len() as isize - 1;
                    while left > prefix as isize
                        && right > prefix as isize
                        && components[0].as_bytes()[left as usize]
                            == pattern[0].as_bytes()[right as usize]
                    {
                        suffix += 1;
                        left -= 1;
                        right -= 1;
                    }
                    return (prefix + suffix) * 2 >= components[0].len();
                }
                let document_path = expressions()[1].replace_all(document_path, "");
                let components = document_path.split('/').collect::<Vec<_>>();
                let mut passed = false;
                let (mut document_index, mut pattern_index) = (0, 0);
                while document_index < components.len() && pattern_index < pattern.len() {
                    if document_index == *parameter && !passed {
                        passed = true;
                        if components.len() >= pattern.len() {
                            document_index += 1;
                        }
                        pattern_index += 1;
                        continue;
                    }
                    if components[document_index].to_lowercase()
                        != pattern[pattern_index].to_lowercase()
                    {
                        return false;
                    }
                    document_index += 1;
                    pattern_index += 1;
                }
                if *parameter >= 2 && pattern[*parameter].len() == PLACEHOLDER.len() {
                    let month = pattern[*parameter - 1].parse::<i64>().unwrap_or(0);
                    let year = pattern[*parameter - 2].parse::<i64>().unwrap_or(0);
                    if (1..=12).contains(&month) && year > 1970 && year < 3000 {
                        return false;
                    }
                }
                true
            }
        }
    }

    pub fn paging_url(&self, value: &str) -> bool {
        match &self.kind {
            Kind::Query(pattern_queries) => {
                let Some(parsed) = Url::request(value) else {
                    return false;
                };
                if !self.valid_for(&parsed) {
                    return false;
                }
                let parsed_queries = queries(&parsed);
                if parsed_queries
                    .keys()
                    .any(|key| !pattern_queries.contains_key(key))
                {
                    return false;
                }
                for (key, values) in pattern_queries {
                    let parameter = values.iter().any(|value| value == PLACEHOLDER);
                    let parsed = parsed_queries.get(key);
                    if !parameter {
                        if parsed != Some(values) {
                            return false;
                        }
                    } else if parsed.is_some_and(|values| {
                        !values.iter().any(|value| value.parse::<i64>().is_ok())
                    }) {
                        return false;
                    }
                }
                true
            }
            Kind::Path {
                start,
                segment,
                prefix,
                suffix,
                ..
            } => {
                if !suffix.is_empty() && !value.ends_with(suffix) {
                    return false;
                }
                let Some(suffix_start) = value.len().checked_sub(suffix.len()) else {
                    return false;
                };
                if self.value.as_bytes()[start - 1] == b'/' {
                    let origin = self.value.find(&self.path).unwrap_or(self.value.len());
                    if let Some(previous) = self.value[..*segment].rfind('/') {
                        if previous >= origin && previous + suffix.len() == value.len() {
                            return value.get(..previous) == self.value.get(..previous);
                        }
                    }
                    if !value.starts_with(prefix) {
                        return false;
                    }
                    let accepted = segment + suffix.len();
                    if accepted == value.len() {
                        return true;
                    }
                    if accepted > value.len() || value.as_bytes().get(*segment) != Some(&b'/') {
                        return false;
                    }
                    value
                        .get(segment + 1..suffix_start)
                        .and_then(|value| value.parse::<i64>().ok())
                        .is_some_and(|number| number >= 0)
                } else {
                    if !value.starts_with(prefix) {
                        return false;
                    }
                    let maximum = (*start).min(suffix_start);
                    let mut different = *segment;
                    while different < maximum
                        && value.as_bytes()[different] == self.value.as_bytes()[different]
                    {
                        different += 1;
                    }
                    if different == suffix_start {
                        if different + 1 == *start
                            && matches!(self.value.as_bytes()[different], b'-' | b'_' | b';' | b',')
                        {
                            return true;
                        }
                        if different + suffix.len() == value.len() {
                            return true;
                        }
                    } else if different == *start {
                        return value
                            .get(different..suffix_start)
                            .and_then(|value| value.parse::<i64>().ok())
                            .is_some_and(|number| number >= 0);
                    }
                    false
                }
            }
        }
    }
}

pub(super) fn query_patterns(url: &Url) -> Vec<Pattern> {
    let original = queries(url);
    let mut patterns = Vec::new();
    for (key, values) in &original {
        if key.is_empty() || bad_name(key) {
            continue;
        }
        for value in values {
            if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                continue;
            }
            let Ok(number) = value.parse() else {
                continue;
            };
            let mut queries = original.clone();
            queries.insert(key.clone(), vec![PLACEHOLDER.into()]);
            let mut url = url.clone();
            url.query = encode_query(&queries);
            let path = path(&url);
            patterns.push(Pattern {
                value: super::unescaped_url(&url, &path),
                number,
                url,
                path,
                kind: Kind::Query(queries),
            });
        }
    }
    patterns
}

fn path_pattern(url: &Url, original: &str, start: usize, end: usize) -> Option<Pattern> {
    if expressions()[1].is_match(&original[end..]) {
        if let Some(capture) = expressions()[2].captures(&original[..start]) {
            if bad_name(&capture[1]) {
                return None;
            }
        }
    }
    let number = original[start..end].parse::<i64>().ok()?;
    if number < 0 {
        return None;
    }
    let path = format!("{}{}{}", &original[..start], PLACEHOLDER, &original[end..]);
    let mut url = url.clone();
    url.query.clear();
    url.clear_fragment();
    let value = super::unescaped_url(&url, &path);
    let start = value.find(PLACEHOLDER)?;
    let segment = value[..start].rfind('/')?;
    let prefix = value[..segment].into();
    let suffix = value[start + PLACEHOLDER.len()..].to_owned();
    let components = path.split('/').collect::<Vec<_>>();
    let parameter = components
        .iter()
        .position(|component| component.contains(PLACEHOLDER))?;
    if parameter == 1 && components.len() == 2 && suffix.is_empty() && start - 1 == segment {
        return None;
    }
    Some(Pattern {
        value,
        number,
        url,
        path,
        kind: Kind::Path {
            parameter,
            start,
            segment,
            prefix,
            suffix,
        },
    })
}

pub(super) fn path_patterns(url: &Url) -> Vec<Pattern> {
    let path = path(url);
    if path.trim_matches('/').is_empty() || !expressions()[0].is_match(path.trim_matches('/')) {
        return Vec::new();
    }
    let mut patterns = expressions()[0]
        .find_iter(&path)
        .filter_map(|digits| path_pattern(url, &path, digits.start(), digits.end()))
        .collect::<Vec<_>>();
    if patterns.is_empty() {
        let path = format!("{}/1", path.trim_end_matches('/'));
        if let Some(pattern) = path_pattern(url, &path, path.len() - 1, path.len()) {
            patterns.push(pattern);
        }
    }
    patterns
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Deserialize)]
    struct Fixture {
        page_patterns: Vec<Case>,
    }

    #[derive(serde::Deserialize)]
    struct Case {
        url: String,
        documents: Vec<String>,
        patterns: Vec<Snapshot>,
    }

    #[derive(Debug, Eq, PartialEq, serde::Deserialize)]
    struct Snapshot {
        kind: String,
        value: String,
        number: i64,
        valid: Vec<bool>,
        paging: Vec<bool>,
    }

    #[test]
    fn pinned_go_page_patterns_parity() {
        let fixture: Fixture =
            serde_json::from_str(include_str!("../../testdata/go-reference.json")).unwrap();
        assert_eq!(fixture.page_patterns.len(), 40);
        for (index, case) in fixture.page_patterns.into_iter().enumerate() {
            let parsed = Url::parse(&case.url).unwrap();
            let mut actual = Vec::new();
            for (kind, patterns) in [
                ("query", query_patterns(&parsed)),
                ("path", path_patterns(&parsed)),
            ] {
                for pattern in patterns {
                    actual.push(Snapshot {
                        kind: kind.into(),
                        value: pattern.value.clone(),
                        number: pattern.number,
                        valid: case
                            .documents
                            .iter()
                            .map(|document| pattern.valid_for(&Url::parse(document).unwrap()))
                            .collect(),
                        paging: case
                            .documents
                            .iter()
                            .map(|document| pattern.paging_url(document))
                            .collect(),
                    });
                }
            }
            actual.sort_by(|left, right| {
                (&left.kind, &left.value, left.number).cmp(&(
                    &right.kind,
                    &right.value,
                    right.number,
                ))
            });
            assert_eq!(actual, case.patterns, "case {index}: {}", case.url);
        }
    }
}
