use crate::{
    dom::{Document, NodeId},
    domutil,
    urlutil::Url,
};
use regex::Regex;
use std::{collections::BTreeSet, sync::LazyLock};

mod number;
mod pattern;

pub use number::find_page_numbers;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PaginationAlgo {
    #[default]
    PrevNext,
    PageNumber,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[cfg_attr(test, derive(serde::Deserialize))]
#[cfg_attr(test, serde(rename_all = "PascalCase"))]
pub struct PaginationInfo {
    pub next_page: String,
    pub prev_page: String,
}

fn patterns() -> &'static [Regex; 8] {
    static PATTERNS: LazyLock<[Regex; 8]> = LazyLock::new(|| {
        [
        r"(?i-u:next|weiter|continue)|[>\x{bb}](?:[^|]|$)",
        r"(?i-u:prev|early|old|new)|[<\x{ab}]",
        r"(?i-u:article|body|content|entry|hentry|main|page|pagination|post|text|blog|story)",
        r"(?i-u:combx|comment|com-|contact|foot|footer|footnote|masthead|media|meta|outbrain|promo|related|shoutbox|sidebar|sponsor|shopping|tags|tool|widget)",
        r"(?i-u:print|archive|comment|discuss|e-?mail|share|reply|all|login|sign|single|as one|article|post)|\x{7bc7}",
        r"(?i-u:pag(e|ing|inat))",
        r"(?i-u:p(a|g|ag)?(e|ing|ination)?[=/][0-9]{1,2}$)",
        r"(?i-u:first|last)",
    ].map(|pattern| Regex::new(pattern).unwrap())
    });
    &PATTERNS
}

fn contains_number(value: &str) -> bool {
    static DIGIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\p{Nd}").unwrap());
    DIGIT.is_match(value)
}

pub(crate) fn starting_number(value: &str) -> Option<i64> {
    let length = value.bytes().take_while(u8::is_ascii_digit).count();
    value[..length].parse().ok()
}

fn unescaped_url(url: &Url, path: &str) -> String {
    let mut url = url.clone();
    url.set_path(path.as_bytes());
    url.unescaped()
}

fn clean_url(url: &Url) -> (String, String) {
    let mut url = url.clone();
    url.clear_fragment();
    let path = String::from_utf8_lossy(&url.path);
    let path = path.strip_suffix('/').unwrap_or(&path).to_owned();
    (unescaped_url(&url, &path), path)
}

fn directory(path: &str) -> String {
    let directory = &path[..path.rfind('/').map_or(0, |index| index + 1)];
    let rooted = directory.starts_with('/');
    let mut components = Vec::new();
    for component in directory.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                if components.last().is_some_and(|last| *last != "..") {
                    components.pop();
                } else if !rooted {
                    components.push(component);
                }
            }
            _ => components.push(component),
        }
    }
    if rooted && !components.is_empty() {
        format!("/{}", components.join("/"))
    } else if !rooted && components.is_empty() {
        ".".into()
    } else {
        components.join("/")
    }
}

fn page_diff(current: &str, link: &str, skip: usize) -> Option<i64> {
    let mut common = 0;
    for index in skip..current.len().min(link.len()) {
        if current.as_bytes()[index] != link.as_bytes()[index] {
            common = index;
            break;
        }
    }
    let current = starting_number(current.get(common..)?)?;
    let link = starting_number(link.get(common..)?)?;
    if current > 0 && link > 0 {
        Some(link.wrapping_sub(current))
    } else {
        None
    }
}

pub fn find_prev_next(document: &Document, root: NodeId, page_url: &str) -> PaginationInfo {
    find_outlinks(document, root, page_url, [true, true])
}

pub fn find_outlink(document: &Document, root: NodeId, page_url: &str, next: bool) -> String {
    let links = find_outlinks(document, root, page_url, [!next, next]);
    if next {
        links.next_page
    } else {
        links.prev_page
    }
}

fn find_outlinks(
    document: &Document,
    root: NodeId,
    page_url: &str,
    directions: [bool; 2],
) -> PaginationInfo {
    let Some(mut parsed) = Url::parse(page_url) else {
        return PaginationInfo::default();
    };
    let (current, path) = clean_url(&parsed);
    parsed.query.clear();
    parsed.clear_fragment();
    let folder = unescaped_url(&parsed, &directory(&path));
    let prefix = unescaped_url(&parsed, "/");
    let current_lower = current.to_lowercase();
    let folder_lower = folder.to_lowercase();
    let prefix_lower = prefix.to_lowercase();
    let expressions = patterns();
    let mut banned = [BTreeSet::new(), BTreeSet::new()];
    let mut candidates = [Vec::new(), Vec::new()];
    let mut ancestor_flags = vec![0u8; document.nodes.len()];
    let mut pending_parents = Vec::new();
    for index in document.tagged(root, "a") {
        let node = &document.nodes[index];
        let href = domutil::absolute_url(node.attr("href"), Some(page_url));
        if Url::request(&href).is_none() {
            continue;
        }
        let Some(parsed) = Url::parse(&href) else {
            continue;
        };
        if !href.to_lowercase().starts_with(&prefix_lower) {
            continue;
        }
        let mut active = directions;
        active[1] &= contains_number(href.get(prefix.len()..).unwrap_or(""));
        let (href, _) = clean_url(&parsed);
        let href_lower = href.to_lowercase();
        if href_lower == current_lower {
            continue;
        }
        active[1] &= href_lower != folder_lower;
        if !active.into_iter().any(|value| value) {
            continue;
        }
        let text = domutil::inner_text(document, index).trim().to_owned();
        if text.len() > 25 {
            continue;
        }
        if expressions[4].is_match(&text) {
            for (direction, active) in active.into_iter().enumerate() {
                if active {
                    banned[direction].insert(href.clone());
                }
            }
            continue;
        }
        active[1] &= contains_number(href.strip_prefix(&folder).unwrap_or(&href));
        if !active.into_iter().any(|value| value) {
            continue;
        }
        let mut score: i64 = if href.starts_with(&folder) { 0 } else { -25 };
        let link_data = format!("{} {} {}", text, node.attr("class"), node.attr("id"));
        let direction_matches = [
            expressions[1].is_match(&link_data),
            expressions[0].is_match(&link_data),
        ];
        if expressions[5].is_match(&link_data) {
            score += 25;
        }
        let first_or_last = expressions[7].is_match(&link_data);
        if expressions[3].is_match(&link_data) || expressions[4].is_match(&link_data) {
            score -= 50;
        }
        let mut flags = 0;
        let mut parent = document.parent_element(index);
        while let Some(index) = parent {
            if ancestor_flags[index] != 0 {
                flags = ancestor_flags[index];
                break;
            }
            pending_parents.push(index);
            parent = document.parent_element(index);
        }
        while let Some(index) = pending_parents.pop() {
            let node = &document.nodes[index];
            let class = node.attr("class");
            let id = node.attr("id");
            if (!class.is_empty() || !id.is_empty()) && flags & 3 != 3 {
                let parent_data = format!("{class} {id}");
                if flags & 1 == 0 && expressions[5].is_match(&parent_data) {
                    flags |= 1;
                }
                if flags & 2 == 0
                    && expressions[3].is_match(&parent_data)
                    && !expressions[2].is_match(&parent_data)
                {
                    flags |= 2;
                }
            }
            ancestor_flags[index] = flags | 4;
        }
        if flags & 1 != 0 {
            score += 25;
        }
        if flags & 2 != 0 {
            score -= 25;
        }
        if expressions[6].is_match(&href) || expressions[5].is_match(&href) {
            score += 25;
        }
        if expressions[4].is_match(&href) {
            score -= 15;
        }
        if text.len() > 10 {
            score -= text.len() as i64;
        }
        let number = text.parse::<i64>().unwrap_or(0);
        let difference = page_diff(&current, &href, prefix.len());
        for (direction, active) in active.into_iter().enumerate() {
            if !active {
                continue;
            }
            let mut direction_score = score;
            if direction_matches[direction] {
                direction_score += 50;
            }
            if first_or_last && !expressions[1 - direction].is_match(&text) {
                direction_score -= 65;
            }
            if direction_matches[1 - direction] {
                direction_score -= 200;
            }
            if number > 0 {
                direction_score += if direction == 1 && number == 1 {
                    -10
                } else {
                    (10 - number).max(0)
                };
            }
            if difference == Some(if direction == 1 { 1 } else { -1 }) {
                direction_score += 25;
            }
            candidates[direction].push((href.clone(), direction_score));
        }
    }
    let mut best = [String::new(), String::new()];
    for (direction, candidates) in candidates.into_iter().enumerate() {
        let mut best_score = 49;
        for (href, score) in candidates {
            if !banned[direction].contains(&href) && score > best_score {
                best[direction] = href;
                best_score = score;
            }
        }
    }
    let [prev_page, next_page] = best;
    PaginationInfo {
        prev_page,
        next_page,
    }
}

#[cfg(test)]
mod tests {
    use super::directory;

    #[test]
    fn pagination_directory_matches_go_path_cleaning() {
        for (path, expected) in [
            ("", "."),
            ("story", "."),
            ("/", ""),
            ("///story", ""),
            ("/news/story", "/news"),
            ("/news//archive/./story", "/news/archive"),
            ("/news/archive/../story", "/news"),
            ("/../../story", ""),
            ("../news/story", "../news"),
            ("news/../../story", ".."),
            (
                "/web/20190717140047/http://example.com/story",
                "/web/20190717140047/http:/example.com",
            ),
        ] {
            assert_eq!(directory(path), expected, "{path:?}");
        }
    }
}
