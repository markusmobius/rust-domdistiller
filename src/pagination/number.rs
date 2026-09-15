use crate::{
    dom::{Document, Kind, NodeId},
    domutil,
    stringutil::WordCounter,
    urlutil::Url,
};
use regex::Regex;
use std::sync::LazyLock;

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub(super) struct PageInfo {
    pub number: i64,
    pub url: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub(super) struct Group {
    pub pages: Vec<PageInfo>,
    pub delta: i64,
}

#[derive(Default)]
pub(super) struct Groups {
    pub groups: Vec<Group>,
}

impl Groups {
    pub fn add_group(&mut self) {
        if self
            .groups
            .last()
            .is_none_or(|group| !group.pages.is_empty())
        {
            self.groups.push(Group::default());
        }
    }

    pub fn add(&mut self, page: PageInfo) {
        let Some(group) = self.groups.last_mut() else {
            return;
        };
        let Some(previous) = group.pages.last().cloned() else {
            group.pages.push(page);
            return;
        };
        let delta = (page.number - previous.number).signum();
        if delta != group.delta {
            if group.delta != 0 {
                self.groups.push(Group {
                    pages: if delta == 0 {
                        Vec::new()
                    } else {
                        vec![previous]
                    },
                    delta: 0,
                });
            }
        } else if delta == 0 {
            group.pages.clear();
        }
        let group = self.groups.last_mut().unwrap();
        group.pages.push(page);
        group.delta = delta;
    }

    pub fn clean_up(&mut self) {
        if self
            .groups
            .last()
            .is_some_and(|group| group.pages.is_empty())
        {
            self.groups.pop();
        }
    }
}

struct Collector<'a> {
    document: &'a Document,
    page_url: &'a str,
    host: Vec<u8>,
    counter: WordCounter,
    groups: Groups,
    forward_links: usize,
}

impl Collector<'_> {
    fn link(&self, index: NodeId) -> Option<PageInfo> {
        let text = domutil::inner_text(self.document, index);
        let text = text.replace(['(', ')', '[', ']', '{', '}'], "");
        let number = text.trim().parse::<i64>().ok()?;
        if !(0..=100).contains(&number) {
            return None;
        }
        let href =
            domutil::absolute_url(self.document.nodes[index].attr("href"), Some(self.page_url));
        if href.is_empty() || href.starts_with("javascript:") {
            return Some(PageInfo { number, url: href });
        }
        if Url::request(&href)?.host != self.host {
            return None;
        }
        let mut parsed = Url::parse(&href)?;
        if parsed.path.ends_with(b"/") {
            parsed.path.pop();
        }
        parsed.raw_path = String::from_utf8_lossy(&parsed.path).into_owned();
        parsed.clear_fragment();
        Some(PageInfo {
            number,
            url: parsed.to_string(),
        })
    }

    fn text(&mut self, text: &str) -> bool {
        static TERMS: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(
                r"(?i)([^\t\n\f\r ]*[A-Za-z0-9_\x{c0}-\x{1fff}\x{2c00}-\x{d7ff}][^\t\n\f\r ]*)",
            )
            .unwrap()
        });
        static DIGITS: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"^[^A-Za-z0-9]*([0-9]+)[^A-Za-z0-9]*$").unwrap());
        if !super::contains_number(text) {
            self.groups.add_group();
            return false;
        }
        let mut added = false;
        for term in TERMS.find_iter(text) {
            let number = DIGITS
                .captures(term.as_str())
                .and_then(|capture| capture[1].parse::<i64>().ok())
                .unwrap_or(-1);
            if (0..=100).contains(&number) {
                self.groups.add(PageInfo {
                    number,
                    url: String::new(),
                });
                added = true;
            } else {
                self.groups.add_group();
            }
        }
        added
    }

    fn closest(&mut self, mut start: NodeId, mut check_start: bool, backward: bool) {
        loop {
            let node = if check_start {
                Some(start)
            } else {
                self.document.nodes[start].parent.and_then(|parent| {
                    let siblings = &self.document.nodes[parent].children;
                    let position = siblings.iter().position(|&index| index == start)?;
                    if backward {
                        position.checked_sub(1).map(|position| siblings[position])
                    } else {
                        siblings.get(position + 1).copied()
                    }
                })
            };
            let Some(index) = node else {
                let Some(parent) = self.document.nodes[start].parent else {
                    return;
                };
                let name = self.document.nodes[parent].tag.to_ascii_lowercase();
                if name.contains("body") || name.contains("html") {
                    return;
                }
                start = parent;
                check_start = false;
                continue;
            };
            check_start = false;
            let node = &self.document.nodes[index];
            if node.kind == Kind::Text {
                if !node.data.is_empty() && self.counter.count(&node.data) != 0 {
                    let added = self.text(&node.data);
                    if backward || !added {
                        return;
                    }
                }
            } else if node.kind == Kind::Element && node.tag == "a" {
                if backward {
                    return;
                }
                self.forward_links += 1;
                if let Some(page) = self.link(index) {
                    self.groups.add(page);
                } else {
                    self.groups.add_group();
                    return;
                }
            } else if !node.children.is_empty() {
                start = if backward {
                    *node.children.last().unwrap()
                } else {
                    node.children[0]
                };
                check_start = true;
                continue;
            }
            start = index;
        }
    }
}

fn collect(document: &Document, root: NodeId, page_url: &str, counter: WordCounter) -> Vec<Group> {
    let Some(parsed) = Url::parse(page_url) else {
        return Vec::new();
    };
    let mut collector = Collector {
        document,
        page_url,
        host: parsed.host,
        counter,
        groups: Groups::default(),
        forward_links: 0,
    };
    let anchors = document.tagged(root, "a");
    let mut position = 0;
    while position < anchors.len() {
        let index = anchors[position];
        let Some(page) = collector.link(index) else {
            position += 1;
            continue;
        };
        collector.groups.add_group();
        collector.closest(index, false, true);
        collector.groups.add(page);
        collector.forward_links = 0;
        collector.closest(index, false, false);
        position += 1 + collector.forward_links;
    }
    collector.groups.clean_up();
    collector.groups.groups
}

#[derive(Clone)]
struct Link {
    number: i64,
    parameter: i64,
    position: usize,
}

struct Parameter {
    pattern: String,
    pages: Vec<PageInfo>,
    linear: bool,
    next: String,
}

fn linear(links: &[Link]) -> bool {
    if links.len() < 2 {
        return false;
    }
    let (first, second) = (&links[0], &links[1]);
    if links.len() == 2 && first.number.max(second.number) > 4 {
        return false;
    }
    let delta_x = second.number.wrapping_sub(first.number);
    if delta_x == 0 {
        return false;
    }
    let coefficient = second
        .parameter
        .wrapping_sub(first.parameter)
        .wrapping_div(delta_x);
    if coefficient == 0 {
        return false;
    }
    let delta = first
        .parameter
        .wrapping_sub(coefficient.wrapping_mul(first.number));
    if delta != 0 && delta != coefficient.wrapping_neg() {
        return false;
    }
    links[2..]
        .iter()
        .all(|link| link.parameter == coefficient.wrapping_mul(link.number).wrapping_add(delta))
}

fn page_sequence(pages: &[PageInfo], next: &mut String) -> bool {
    if pages.len() <= 1 || (pages[0].number != 1 && pages[0].url.is_empty()) {
        return false;
    }
    let mut plain = false;
    for page in pages {
        if page.url.is_empty() {
            if plain {
                return false;
            }
            plain = true;
        } else if plain && next.is_empty() {
            next.clone_from(&page.url);
        }
    }
    if pages.len() == 2 {
        return pages[0].number + 1 == pages[1].number;
    }
    let mut sequences = 1;
    let mut current_length = 1;
    let mut longest = 1;
    for pair in pages.windows(2) {
        if pair[0].number + 1 == pair[1].number {
            current_length += 1;
        } else {
            sequences += 1;
            current_length = 1;
        }
        longest = longest.max(current_length);
    }
    sequences <= 2 && longest > 1
}

fn page_state(links: &[Link], pages: &[PageInfo]) -> Option<String> {
    let first = links.first()?.position;
    let mut last = None;
    let mut gap = None;
    let mut parameters = std::collections::BTreeSet::new();
    for link in links {
        if let Some(previous) = last {
            if link.position != previous + 1 {
                if link.position <= previous || link.position != previous + 2 || gap.is_some() {
                    return None;
                }
                gap = Some(link.position - 1);
            }
        }
        if !parameters.insert(link.parameter) {
            return None;
        }
        last = Some(link.position);
    }
    if let Some(gap) = gap {
        if gap == 0 || gap >= pages.len() - 1 {
            return None;
        }
        return if pages[gap - 1].number == pages[gap].number - 1
            && pages[gap + 1].number == pages[gap].number + 1
        {
            Some(pages[gap + 1].url.clone())
        } else {
            None
        };
    }
    let last = last?;
    if (first <= 1 && pages[0].number == 1 && pages[1].number == 2)
        || (first == 2
            && pages[2].number == 3
            && pages[1].url.is_empty()
            && !pages[0].url.is_empty())
        || ((last == pages.len() - 1 || last == pages.len() - 2)
            && pages[pages.len() - 2].number + 1 == pages[pages.len() - 1].number)
        || (first + 1..last).any(|index| pages[index - 1].number + 2 == pages[index + 1].number)
    {
        return Some(String::new());
    }
    None
}

fn evaluate(
    pattern: &super::pattern::Pattern,
    links: &[Link],
    pages: &mut [PageInfo],
    first_url: &str,
) -> Option<Parameter> {
    if links.len() >= 2 {
        let mut next = page_state(links, pages)?;
        if !page_sequence(pages, &mut next) {
            return None;
        }
        return Some(Parameter {
            pattern: pattern.value.clone(),
            pages: links
                .iter()
                .map(|link| PageInfo {
                    number: link.number,
                    url: pages[link.position].url.clone(),
                })
                .collect(),
            linear: linear(links),
            next,
        });
    }
    if links.len() == 1 && !first_url.is_empty() {
        let link = &links[0];
        let second = link.number == 2 && link.position == 1;
        let third = link.number == 3 && link.position == 2;
        pages[1].number = 2;
        if pages[0].number == 1 && (second || third) && pattern.paging_url(first_url) {
            return Some(Parameter {
                pattern: pattern.value.clone(),
                pages: vec![
                    PageInfo {
                        number: 1,
                        url: first_url.into(),
                    },
                    PageInfo {
                        number: link.number,
                        url: pages[link.position].url.clone(),
                    },
                ],
                linear: true,
                next: if third {
                    pages[link.position].url.clone()
                } else {
                    String::new()
                },
            });
        }
    }
    None
}

fn compare(best: &mut Option<Parameter>, candidate: Parameter) {
    if best
        .as_ref()
        .is_none_or(|best| !best.linear && candidate.linear)
    {
        *best = Some(candidate);
    }
}

fn detect(
    mut group: Group,
    document: &Url,
    document_url: &str,
    document_path: &str,
    accepted: &str,
) -> Option<Parameter> {
    let mut outlinks = group
        .pages
        .iter()
        .filter(|page| !page.url.is_empty())
        .count();
    if outlinks == 0 {
        return None;
    }
    if group.delta < 0 {
        group.pages.reverse();
    }
    let pages = &mut group.pages;
    if pages.len() == 2 && outlinks == 1 && pages[0].number == 1 && pages[1].number == 2 {
        let index = usize::from(!pages[0].url.is_empty());
        pages[index].url = document_url.into();
        outlinks += 1;
    }
    if outlinks < 2 {
        return None;
    }
    let mut date = 0;
    for page in pages.iter() {
        if page.number == date + 1 {
            date += 1;
        }
    }
    if (28..=31).contains(&date) {
        return None;
    }
    let mut candidates =
        std::collections::BTreeMap::<String, (super::pattern::Pattern, Vec<Link>)>::new();
    let mut parsed = Vec::new();
    let mut first_url = String::new();
    for page in pages.iter() {
        let parsed_url = Url::request(&page.url)
            .and_then(|_| Url::parse(&page.url))
            .map(|mut parsed| {
                parsed.user = None;
                parsed.clear_fragment();
                if page.number == 1 {
                    first_url.clone_from(&page.url);
                }
                parsed
            });
        parsed.push(parsed_url);
    }
    for queries in [true, false] {
        if !candidates.is_empty() {
            break;
        }
        for (position, parsed) in parsed.iter().enumerate() {
            let Some(parsed) = parsed else {
                continue;
            };
            let patterns = if queries {
                super::pattern::query_patterns(parsed)
            } else {
                super::pattern::path_patterns(parsed)
            };
            for pattern in patterns {
                let link = Link {
                    number: pages[position].number,
                    parameter: pattern.number,
                    position,
                };
                candidates
                    .entry(pattern.value.clone())
                    .or_insert_with(|| (pattern, Vec::new()))
                    .1
                    .push(link);
            }
        }
    }
    let mut best = None;
    for (value, (pattern, links)) in candidates {
        if value == accepted
            || links.len() > 100
            || !pattern.valid_for_path(document, document_path)
        {
            continue;
        }
        let Some(mut candidate) = evaluate(&pattern, &links, pages, &first_url) else {
            continue;
        };
        let document_url = document_url.strip_suffix('/').unwrap_or(document_url);
        let can_insert =
            candidate.pages.len() >= 2
                && candidate.pages[0].number != 1
                && document_url.len() < candidate.pages[0].url.len()
                && candidate.pages.iter().enumerate().all(|(index, page)| {
                    page.number == index as i64 + 2 && page.url != document_url
                })
                && !pages.iter().any(|page| {
                    page.number == 1 && !page.url.is_empty() && page.url != document_url
                });
        if can_insert
            || (pattern.paging_url(document_url)
                && candidate.pages[0].number == 2
                && candidate.pages[0].url != document_url
                && document_url.len() < candidate.pages[0].url.len())
        {
            candidate.pages.insert(
                0,
                PageInfo {
                    number: 1,
                    url: document_url.into(),
                },
            );
        }
        compare(&mut best, candidate);
    }
    best
}

pub fn find_page_numbers(
    document: &Document,
    root: NodeId,
    page_url: &str,
    counter: WordCounter,
) -> super::PaginationInfo {
    let mut result = super::PaginationInfo::default();
    let Some(mut parsed) = Url::parse(page_url) else {
        return result;
    };
    if parsed.path.ends_with(b"/") {
        parsed.path.pop();
    }
    parsed.raw_path = String::from_utf8_lossy(&parsed.path).into_owned();
    let current = parsed.unescaped();
    let document_url = parsed.to_string();
    let groups = collect(document, root, &document_url, counter);
    let Some(mut parsed) = Url::request(&document_url)
        .filter(|url| !url.scheme.is_empty() && !url.hostname().is_empty())
    else {
        return result;
    };
    parsed.user = None;
    let detection_url = parsed.to_string();
    let path = String::from_utf8_lossy(&parsed.path).into_owned();
    let mut best: Option<Parameter> = None;
    for group in groups.into_iter().filter(|group| group.pages.len() >= 2) {
        let accepted = best.as_ref().map_or("", |best| best.pattern.as_str());
        if let Some(candidate) = detect(group, &parsed, &detection_url, &path, accepted) {
            compare(&mut best, candidate);
        }
    }
    let Some(mut best) = best else {
        return result;
    };
    if best.next.is_empty() {
        if let Some(position) = best.pages.iter().position(|page| page.url == document_url) {
            if let Some(page) = best.pages.get(position + 1) {
                best.next.clone_from(&page.url);
            }
        }
    }
    result.next_page = best.next;
    if result.next_page.is_empty() {
        if let Some(page) = best.pages.iter().rev().find(|page| page.url != current) {
            result.prev_page.clone_from(&page.url);
        }
    } else if let Some(position) = best
        .pages
        .iter()
        .position(|page| page.url == result.next_page)
    {
        if let Some(page) = best.pages[..position]
            .iter()
            .rev()
            .find(|page| page.url.is_empty() || page.url != current)
        {
            result.prev_page.clone_from(&page.url);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Deserialize)]
    struct Fixture {
        number_groups: Vec<Case>,
    }

    #[derive(serde::Deserialize)]
    struct Case {
        input: Vec<Option<i64>>,
        groups: Vec<Group>,
    }

    #[test]
    fn pinned_go_number_groups_parity() {
        let fixture: Fixture =
            serde_json::from_str(include_str!("../../testdata/go-reference.json")).unwrap();
        assert_eq!(fixture.number_groups.len(), 240);
        for (index, case) in fixture.number_groups.into_iter().enumerate() {
            let mut groups = Groups::default();
            for number in &case.input {
                if let Some(number) = number {
                    groups.add(PageInfo {
                        number: *number,
                        url: format!("page{number}"),
                    });
                } else {
                    groups.add_group();
                }
            }
            groups.clean_up();
            assert_eq!(groups.groups, case.groups, "case {index}: {:?}", case.input);
        }
    }
}
