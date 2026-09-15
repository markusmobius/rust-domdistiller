use crate::{
    dom::{Document, NodeId},
    label,
    stringutil::WordCounter,
    webdoc::{TextBlock, WebDocument},
};
use regex::Regex;
use std::{collections::BTreeSet, sync::LazyLock};

pub fn classify_num_words(blocks: &mut [TextBlock]) -> bool {
    let mut changed = false;
    for index in 0..blocks.len() {
        let previous = index.checked_sub(1).map(|index| &blocks[index]);
        let current = &blocks[index];
        let next = blocks.get(index + 1);
        let is_content = if current.link_density <= 0.333333 {
            if previous.is_none_or(|block| block.link_density <= 0.555556) {
                if current.num_words <= 16 {
                    if next.is_none_or(|block| block.num_words <= 15) {
                        previous.is_some_and(|block| block.num_words > 4)
                    } else {
                        true
                    }
                } else {
                    true
                }
            } else if current.num_words <= 40 {
                next.is_some_and(|block| block.num_words > 17)
            } else {
                true
            }
        } else {
            false
        };
        changed |= blocks[index].set_content(is_content);
    }
    changed
}

pub fn label_to_boilerplate(blocks: &mut [TextBlock], labels: &[&str]) -> bool {
    let mut changed = false;
    for block in blocks {
        if block.is_content && labels.iter().any(|label| block.labels.contains(*label)) {
            changed |= block.set_content(false);
        }
    }
    changed
}

pub fn boilerplate_blocks(blocks: &mut Vec<TextBlock>, label_to_keep: &str) -> bool {
    let old_len = blocks.len();
    blocks.retain(|block| {
        block.is_content || (label_to_keep.is_empty() && block.labels.contains(label::TITLE))
    });
    old_len != blocks.len()
}

pub fn heading_fusion(blocks: &mut Vec<TextBlock>) -> bool {
    let mut changed = false;
    let mut index = 1;
    while index < blocks.len() {
        let previous = &blocks[index - 1];
        let current = &blocks[index];
        if previous.labels.contains(label::HEADING)
            && ![previous, current].iter().any(|block| {
                block.labels.contains(label::STRICTLY_NOT_CONTENT)
                    || block.labels.contains(label::TITLE)
            })
        {
            if current.is_content {
                let heading_was_content = previous.is_content;
                let current = blocks.remove(index);
                let previous = &mut blocks[index - 1];
                previous.merge_next(current);
                previous.labels.remove(label::HEADING);
                if !heading_was_content {
                    previous
                        .labels
                        .insert(label::BOILERPLATE_HEADING_FUSED.into());
                }
                changed = true;
                continue;
            } else if previous.is_content {
                changed |= blocks[index - 1].set_content(false);
            }
        }
        index += 1;
    }
    changed
}

pub fn block_proximity_fusion(blocks: &mut Vec<TextBlock>, post_filtering: bool) -> bool {
    let mut changed = false;
    let mut previous_index = 0;
    let mut index = 1;
    while index < blocks.len() {
        let previous = &blocks[previous_index];
        let current = &blocks[index];
        if !current.is_content || !previous.is_content {
            previous_index = index;
            index += 1;
            continue;
        }
        if current.offset_start - previous.offset_end - 1 <= 1 {
            let allowed = if post_filtering {
                previous.tag_level == current.tag_level
            } else {
                !current.labels.contains(label::BOILERPLATE_HEADING_FUSED)
            } && [label::STRICTLY_NOT_CONTENT, label::TITLE]
                .iter()
                .all(|label| previous.labels.contains(*label) == current.labels.contains(*label));
            if allowed {
                let current = blocks.remove(index);
                blocks[previous_index].merge_next(current);
                changed = true;
                continue;
            }
        } else {
            previous_index = index;
        }
        index += 1;
    }
    changed
}

pub fn keep_largest_block(blocks: &mut [TextBlock]) -> bool {
    if blocks.len() < 2 {
        return false;
    }
    let mut largest = None;
    for (index, block) in blocks.iter().enumerate() {
        if block.is_content
            && largest.is_none_or(|old: usize| block.num_words > blocks[old].num_words)
        {
            largest = Some(index);
        }
    }
    for (index, block) in blocks.iter_mut().enumerate() {
        let selected = largest == Some(index);
        block.set_content(selected);
        block.labels.insert(
            if selected {
                label::VERY_LIKELY_CONTENT
            } else {
                label::MIGHT_BE_CONTENT
            }
            .into(),
        );
    }
    true
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SiblingOptions {
    pub cross_titles: bool,
    pub cross_headings: bool,
    pub mixed_tags: bool,
    pub max_link_density: f64,
    pub max_distance: usize,
}

fn endpoint(block: &TextBlock, model: &WebDocument, first: bool) -> NodeId {
    let text = &model.texts[block.elements[0]];
    if first {
        text.first_word
    } else {
        text.last_word
    }
}

pub(crate) fn similar_siblings(
    blocks: &mut [TextBlock],
    model: &WebDocument,
    document: &Document,
    options: SiblingOptions,
) -> bool {
    if blocks.len() < 2 {
        return false;
    }
    let representatives: Vec<_> = blocks
        .iter()
        .enumerate()
        .map(|(index, block)| {
            let previous = index
                .checked_sub(1)
                .map(|previous| endpoint(&blocks[previous], model, false));
            let next = blocks
                .get(index + 1)
                .map(|next| endpoint(next, model, true));
            let mut current = endpoint(block, model, true);
            while let Some(parent) = document.nodes[current].parent {
                if document.contains(Some(parent), previous)
                    || document.contains(Some(parent), next)
                {
                    break;
                }
                current = parent;
            }
            current
        })
        .collect();
    let similar = |left: usize, right: usize| {
        let left = &document.nodes[representatives[left]];
        let right = &document.nodes[representatives[right]];
        (options.mixed_tags || (left.kind == right.kind && left.tag == right.tag))
            && left.parent == right.parent
    };
    let mut bad = vec![0; blocks.len()];
    let mut good = vec![0; blocks.len()];
    let (mut bad_begin, mut bad_end, mut good_begin, mut good_end) = (0, 0, 0, 0);
    let mut changed = false;
    for index in 0..blocks.len() {
        let block = &blocks[index];
        let title = block.labels.contains(label::TITLE);
        let heading = block.labels.contains(label::HEADING);
        let strict = block.labels.contains(label::STRICTLY_NOT_CONTENT);
        if (!options.cross_titles && title) || (!options.cross_headings && heading) {
            good_begin = good_end;
            bad_begin = bad_end;
            continue;
        }
        if block.is_content && !strict && !title {
            good[good_end] = index;
            good_end += 1;
            let candidates = bad_begin..bad_end;
            for candidate_index in candidates {
                let candidate = bad[candidate_index];
                if index - candidate > options.max_distance {
                    if candidate_index == bad_begin {
                        bad_begin += 1;
                    }
                    continue;
                }
                if similar(index, candidate) {
                    changed = true;
                    blocks[candidate].set_content(true);
                    bad[candidate_index] = bad[bad_begin];
                    bad_begin += 1;
                }
            }
        } else if block.link_density <= options.max_link_density
            && !block.is_content
            && !strict
            && !title
        {
            let mut candidate_index = good_begin;
            while candidate_index < good_end {
                let candidate = good[candidate_index];
                if index - candidate > options.max_distance {
                    if candidate_index == good_begin {
                        good_begin += 1;
                    }
                } else if similar(index, candidate) {
                    changed = true;
                    blocks[index].set_content(true);
                    good[candidate_index] = good[good_begin];
                    good_begin += 1;
                    break;
                }
                candidate_index += 1;
            }
            if candidate_index == good_end {
                bad[bad_end] = index;
                bad_end += 1;
            } else {
                good[good_end] = index;
                good_end += 1;
            }
        }
    }
    changed
}

pub(crate) fn keep_largest_with_siblings(
    blocks: &mut [TextBlock],
    model: &WebDocument,
    document: &Document,
) -> bool {
    let changed = keep_largest_block(blocks);
    if !changed {
        return false;
    }
    let Some(largest) = blocks.iter().position(|block| block.is_content) else {
        return true;
    };
    let parent = |node: NodeId| document.parent_element(node);
    let grandparent = |node: Option<NodeId>| node.and_then(|node| document.parent_element(node));
    let mut first = parent(endpoint(&blocks[largest], model, true));
    for index in (0..largest).rev() {
        let last = parent(endpoint(&blocks[index], model, false));
        if grandparent(first) == grandparent(last) {
            blocks[index].set_content(true);
            blocks[index]
                .labels
                .insert(label::SIBLING_OF_MAIN_CONTENT.into());
            first = parent(endpoint(&blocks[index], model, true));
        }
    }
    let mut last = parent(endpoint(&blocks[largest], model, false));
    for block in &mut blocks[largest + 1..] {
        let first = parent(endpoint(block, model, true));
        if grandparent(last) == grandparent(first) {
            block.set_content(true);
            block.labels.insert(label::SIBLING_OF_MAIN_CONTENT.into());
            last = parent(endpoint(block, model, false));
        }
    }
    true
}

pub(crate) fn article(
    blocks: &mut Vec<TextBlock>,
    model: &WebDocument,
    document: &Document,
    counter: WordCounter,
    titles: &[String],
) {
    terminating_blocks(blocks);
    document_title_match(blocks, counter, titles);
    classify_num_words(blocks);
    label_to_boilerplate(blocks, &[label::STRICTLY_NOT_CONTENT]);
    similar_siblings(
        blocks,
        model,
        document,
        SiblingOptions {
            cross_headings: true,
            max_link_density: 0.5,
            max_distance: 10,
            ..SiblingOptions::default()
        },
    );
    similar_siblings(
        blocks,
        model,
        document,
        SiblingOptions {
            cross_headings: true,
            mixed_tags: true,
            max_distance: 10,
            ..SiblingOptions::default()
        },
    );
    heading_fusion(blocks);
    block_proximity_fusion(blocks, false);
    boilerplate_blocks(blocks, label::TITLE);
    block_proximity_fusion(blocks, true);
    keep_largest_with_siblings(blocks, model, document);
    expand_title_to_content(blocks);
    large_block_around_level(blocks);
    list_at_end(blocks);
}

pub fn expand_title_to_content(blocks: &mut [TextBlock]) -> bool {
    let mut title = None;
    let mut content_start = None;
    for (index, block) in blocks.iter().enumerate() {
        if content_start.is_none() && block.labels.contains(label::TITLE) {
            title = Some(index);
        }
        if content_start.is_none() && block.is_content {
            content_start = Some(index);
        }
    }
    let (Some(title), Some(content_start)) = (title, content_start) else {
        return false;
    };
    if content_start <= title {
        return false;
    }
    let mut changed = false;
    for block in &mut blocks[title..content_start] {
        if block.labels.contains(label::MIGHT_BE_CONTENT) {
            changed |= block.set_content(true);
        }
    }
    changed
}

pub fn large_block_around_level(blocks: &mut [TextBlock]) -> bool {
    let level = blocks
        .iter()
        .find(|block| block.is_content && block.labels.contains(label::VERY_LIKELY_CONTENT))
        .map_or(-1, |block| block.tag_level);
    if level == -1 {
        return false;
    }
    let mut changed = false;
    for block in blocks {
        if !block.is_content && block.num_words >= 100 && (block.tag_level - level).abs() <= 1 {
            changed |= block.set_content(true);
        }
    }
    changed
}

pub fn list_at_end(blocks: &mut [TextBlock]) -> bool {
    let mut changed = false;
    let mut level = i16::MAX as i32;
    for block in blocks {
        if block.is_content && block.labels.contains(label::VERY_LIKELY_CONTENT) {
            level = block.tag_level;
        } else if block.tag_level > level
            && block.labels.contains(label::MIGHT_BE_CONTENT)
            && block.labels.contains(label::LI)
            && block.link_density == 0.0
        {
            block.set_content(true);
            changed = true;
        } else {
            level = i16::MAX as i32;
        }
    }
    changed
}

pub fn terminating_blocks(blocks: &mut [TextBlock]) -> bool {
    static PATTERN: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)(^(comments|\x{a9} reuters|please rate this|post a comment|[0-9]+[\t\n\f\r ]+(comments|users responded in))|what you think\.\.\.|add your comment|add comment|reader views|have your say|reader comments|r\x{e4}tta artikeln|^thanks for your comments - this feedback is now closed$)").unwrap()
    });
    let mut changed = false;
    for block in blocks {
        if block.num_words > 14 {
            continue;
        }
        let text = block.text.trim();
        let terminates = if text.chars().count() >= 8 {
            PATTERN.is_match(text)
        } else if block.link_density == 1.0 {
            text == "Comment"
        } else {
            text == "Shares"
        };
        if terminates {
            block.labels.insert(label::STRICTLY_NOT_CONTENT.into());
            changed = true;
        }
    }
    changed
}

pub fn document_title_match(
    blocks: &mut [TextBlock],
    counter: WordCounter,
    titles: &[String],
) -> bool {
    static SPLITS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
        [
            r"[ ]*[|\x{bb}-][ ]*",
            r"[ ]*[|\x{bb}:][ ]*",
            r"[ ]*[|\x{bb}:()][ ]*",
            r"[ ]*[|\x{bb}:()\-][ ]*",
            r"[ ]*[|\x{bb},:()\-][ ]*",
            r"[ ]*[|\x{bb},:()\-\x{a0}][ ]*",
        ]
        .iter()
        .map(|pattern| Regex::new(pattern).unwrap())
        .collect()
    });
    static PARTS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
        [r"[ ]+\|[ ]+", r"[ ]+-[ ]+"]
            .iter()
            .map(|pattern| Regex::new(pattern).unwrap())
            .collect()
    });
    static REPLACEMENTS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
        [r" - [^\-]+$", r"^[^\-]+ - "]
            .iter()
            .map(|pattern| Regex::new(pattern).unwrap())
            .collect()
    });
    let normalize = |text: &str| {
        text.replace('\u{a0}', " ")
            .replace('\'', "")
            .trim()
            .to_lowercase()
    };
    let mut potential = BTreeSet::new();
    for title in titles {
        let title = normalize(title);
        if title.is_empty() || potential.contains(&title) {
            continue;
        }
        for pattern in SPLITS.iter() {
            let parts: Vec<_> = pattern.split(&title).collect();
            if parts.len() == 1 {
                continue;
            }
            let (mut longest, mut most_words, mut longest_length) = ("", 0, 0);
            for part in parts {
                if part.contains(".com") {
                    continue;
                }
                let words = counter.count(part);
                let length = part.chars().count();
                if words > most_words || length > longest_length {
                    longest = part;
                    most_words = words;
                    longest_length = length;
                }
            }
            if longest_length > 0 {
                potential.insert(longest.trim().to_owned());
            }
        }
        for pattern in PARTS.iter() {
            let parts: Vec<_> = pattern.split(&title).collect();
            if parts.len() > 1 {
                for part in parts {
                    if !part.contains(".com") && counter.count(part) >= 4 {
                        potential.insert(part.to_owned());
                    }
                }
            }
        }
        for pattern in REPLACEMENTS.iter() {
            potential.insert(pattern.replace_all(&title, "").into_owned());
        }
    }
    let mut changed = false;
    for block in blocks {
        let text = normalize(&block.text);
        let stripped: String = text
            .chars()
            .filter(|character| !matches!(character, '?' | '!' | '.' | '-' | ':'))
            .collect();
        if potential.contains(&text) || potential.contains(stripped.trim()) {
            block.labels.insert(label::TITLE.into());
            changed = true;
        }
    }
    changed
}
