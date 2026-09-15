use crate::dom::NodeId;
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Text {
    pub text: String,
    pub labels: BTreeSet<String>,
    pub num_words: usize,
    pub num_linked_words: usize,
    pub tag_level: i32,
    pub group: i32,
    pub nodes: Vec<NodeId>,
    pub first_word: NodeId,
    pub last_word: NodeId,
    pub element: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ElementKind {
    Text(usize),
    Tag {
        name: String,
        start: bool,
    },
    Image {
        node: NodeId,
        caption: Option<NodeId>,
    },
    Table(NodeId),
    Video(NodeId),
    Embed {
        node: NodeId,
        kind: String,
        id: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Element {
    pub kind: ElementKind,
    pub is_content: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct WebDocument {
    pub texts: Vec<Text>,
    pub elements: Vec<Element>,
    pub page_url: Option<String>,
}

impl WebDocument {
    pub fn retain_lead_image(&mut self, document: &crate::Document) -> bool {
        let content = self
            .elements
            .iter()
            .enumerate()
            .filter_map(|(index, element)| {
                if !element.is_content {
                    return None;
                }
                if let ElementKind::Text(text) = element.kind {
                    Some((index, text))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let (Some(&(_, first)), Some(&(last, _))) = (content.first(), content.last()) else {
            return false;
        };
        let first_node = self.texts[first].first_word;
        let depth = |node: NodeId| {
            let mut depth = 0;
            let mut current = document.nodes[node].parent;
            while let Some(parent) = current {
                depth += 1;
                current = document.nodes[parent].parent;
            }
            depth
        };
        let mut best = None;
        let mut best_score = 0;
        for (index, element) in self.elements.iter().enumerate() {
            if index == last
                || (matches!(element.kind, ElementKind::Image { .. }) && element.is_content)
            {
                break;
            }
            if let ElementKind::Image { node, .. } = element.kind {
                let mut ancestor = Some(first_node);
                while ancestor
                    .is_some_and(|ancestor| !document.contains(Some(ancestor), Some(node)))
                {
                    ancestor = ancestor.and_then(|node| document.nodes[node].parent);
                }
                let difference = depth(first_node) - ancestor.map_or(-1, depth);
                let mut score = if difference < 4 {
                    25
                } else if difference < 6 {
                    15
                } else if difference < 8 {
                    5
                } else {
                    0
                };
                if document.nodes[node].tag == "figure" || document.has_ancestor(node, &["figure"])
                {
                    score += 15;
                }
                if score > 13 && (best.is_none() || score > best_score) {
                    best = Some(index);
                    best_score = score;
                }
            }
        }
        if let Some(index) = best {
            self.elements[index].is_content = true;
            true
        } else {
            false
        }
    }

    pub fn image_urls(&self, document: &crate::Document) -> Vec<String> {
        let mut urls = Vec::new();
        for element in self.elements.iter().filter(|element| element.is_content) {
            if let ElementKind::Image { node, .. } = element.kind {
                urls.extend(crate::embed::image_urls(
                    document,
                    node,
                    self.page_url.as_deref(),
                ));
            } else if let ElementKind::Table(node) = element.kind {
                urls.extend(crate::table::image_urls(
                    document,
                    node,
                    self.page_url.as_deref(),
                ));
            }
        }
        urls
    }

    pub fn retain_relevant_elements(&mut self) -> bool {
        let mut in_content = false;
        let mut changed = false;
        for element in &mut self.elements {
            if element.is_content {
                in_content = true;
            } else if matches!(element.kind, ElementKind::Text(_)) {
                in_content = false;
            } else if in_content {
                element.is_content = true;
                changed = true;
            }
        }
        changed
    }

    pub fn retain_nested_elements(&mut self) {
        let mut is_content = false;
        let mut stack_mark = -1;
        let mut stack = Vec::<usize>::new();
        for index in 0..self.elements.len() {
            match &self.elements[index].kind {
                ElementKind::Tag { start: true, .. } => {
                    self.elements[index].is_content = is_content;
                    stack.push(index);
                    is_content = false;
                }
                ElementKind::Tag { start: false, .. } => {
                    let Some(start) = stack.pop() else {
                        continue;
                    };
                    is_content |= stack_mark >= stack.len() as i32;
                    if is_content {
                        stack_mark = stack.len() as i32 - 1;
                    }
                    let was_content = self.elements[start].is_content;
                    self.elements[start].is_content = is_content;
                    self.elements[index].is_content = is_content;
                    is_content = was_content;
                }
                _ => is_content |= self.elements[index].is_content,
            }
        }
    }

    pub fn output(&self, document: &crate::Document, text_only: bool) -> String {
        let mut output = String::new();
        for element in self.elements.iter().filter(|element| element.is_content) {
            match &element.kind {
                ElementKind::Text(index) => output.push_str(&crate::domutil::text_output(
                    document,
                    &self.texts[*index],
                    text_only,
                )),
                ElementKind::Image { node, caption } => {
                    output.push_str(&crate::embed::image_output(
                        document,
                        *node,
                        *caption,
                        self.page_url.as_deref(),
                        text_only,
                    ))
                }
                ElementKind::Table(node) => output.push_str(&crate::table::output(
                    document,
                    *node,
                    self.page_url.as_deref(),
                    text_only,
                )),
                ElementKind::Video(node) if !text_only => output.push_str(
                    &crate::embed::video_output(document, *node, self.page_url.as_deref()),
                ),
                ElementKind::Embed { node, kind, id } if !text_only => {
                    output.push_str(&crate::embed::site_output(document, *node, kind, id))
                }
                ElementKind::Tag { name, start } if !text_only => {
                    output.push('<');
                    if !start {
                        output.push('/');
                    }
                    output.push_str(name);
                    output.push('>');
                }
                _ => {}
            }
            if text_only {
                output.push('\n');
            }
        }
        output
    }

    pub fn create_text_blocks(&mut self) -> Vec<TextBlock> {
        let mut blocks = Vec::<TextBlock>::new();
        let mut previous_group = None;
        for (index, text) in self.texts.iter_mut().enumerate() {
            if previous_group != Some(text.group) {
                blocks.push(TextBlock {
                    offset_start: index as i32,
                    ..TextBlock::default()
                });
                previous_group = Some(text.group);
            }
            let block = blocks.last_mut().unwrap();
            block.text.push_str(&text.text);
            block.num_words += text.num_words;
            block.num_words_in_anchor += text.num_linked_words;
            block.labels.extend(std::mem::take(&mut text.labels));
            block.elements.push(index);
            block.offset_end = index as i32;
            if block.tag_level == -1 {
                block.tag_level = text.tag_level;
            }
            block.link_density = if block.num_words == 0 {
                0.0
            } else {
                block.num_words_in_anchor as f64 / block.num_words as f64
            };
        }
        blocks
    }

    pub fn apply_blocks(&mut self, blocks: &[TextBlock]) {
        for block in blocks.iter().filter(|block| block.is_content) {
            for &index in &block.elements {
                let text = &mut self.texts[index];
                self.elements[text.element].is_content = true;
                if block.labels.contains(crate::label::TITLE) {
                    text.labels.insert(crate::label::TITLE.into());
                }
            }
        }
    }
}

pub(crate) fn can_be_nested(tag: &str) -> bool {
    matches!(tag, "ul" | "ol" | "li" | "blockquote" | "pre")
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextBlock {
    pub text: String,
    pub labels: BTreeSet<String>,
    pub num_words: usize,
    pub num_words_in_anchor: usize,
    pub link_density: f64,
    pub tag_level: i32,
    pub is_content: bool,
    pub elements: Vec<usize>,
    pub offset_start: i32,
    pub offset_end: i32,
}

impl Default for TextBlock {
    fn default() -> Self {
        Self {
            text: String::new(),
            labels: BTreeSet::new(),
            num_words: 0,
            num_words_in_anchor: 0,
            link_density: 0.0,
            tag_level: -1,
            is_content: false,
            elements: Vec::new(),
            offset_start: -1,
            offset_end: -1,
        }
    }
}

impl TextBlock {
    pub fn set_content(&mut self, value: bool) -> bool {
        let changed = self.is_content != value;
        self.is_content = value;
        changed
    }

    pub fn merge_next(&mut self, other: Self) {
        self.text.push('\n');
        self.text.push_str(&other.text);
        self.num_words += other.num_words;
        self.num_words_in_anchor += other.num_words_in_anchor;
        self.link_density = if self.num_words == 0 {
            0.0
        } else {
            self.num_words_in_anchor as f64 / self.num_words as f64
        };
        self.is_content |= other.is_content;
        self.labels.extend(other.labels);
        if self.elements.is_empty() && !other.elements.is_empty() {
            self.offset_start = other.offset_start;
        }
        if !other.elements.is_empty() {
            self.offset_end = other.offset_end;
        }
        self.elements.extend(other.elements);
        self.tag_level = self.tag_level.min(other.tag_level);
    }
}
