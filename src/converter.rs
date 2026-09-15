use crate::{
    dom::{Document, Kind, NodeId},
    domutil, label,
    stringutil::WordCounter,
    webdoc::{can_be_nested, Element, ElementKind, Text, WebDocument},
};
use regex::Regex;
use std::{collections::BTreeSet, sync::LazyLock};

#[derive(Default)]
struct Action {
    flush: bool,
    anchor: bool,
    changes_level: bool,
    labels: Vec<&'static str>,
}

fn action(document: &Document, index: NodeId) -> Action {
    static COMMENT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)(?-u:\b)comments?(?-u:\b)").unwrap());
    let node = &document.nodes[index];
    let mut action = Action::default();
    match domutil::display(document, index) {
        "none" | "inline" => {}
        "inline-block" | "inline-flex" => action.changes_level = true,
        _ => {
            action.changes_level = true;
            action.flush = true;
        }
    }
    if document.has_ancestor(index, &["li", "summary"]) {
        action.flush = false;
        action.changes_level = false;
    }
    if !matches!(node.tag.as_str(), "html" | "body" | "article") {
        let class = node.attr("class");
        if (COMMENT.is_match(node.attr("id")) || COMMENT.is_match(class))
            && class.split_whitespace().count() <= 2
        {
            action.labels.push(label::STRICTLY_NOT_CONTENT);
        }
        match node.tag.as_str() {
            "aside" | "nav" => action.labels.push(label::STRICTLY_NOT_CONTENT),
            "li" => action.labels.push(label::LI),
            "h1" => action.labels.extend([label::H1, label::HEADING]),
            "h2" => action.labels.extend([label::H2, label::HEADING]),
            "h3" => action.labels.extend([label::H3, label::HEADING]),
            "h4" | "h5" | "h6" => action.labels.push(label::HEADING),
            "a" => {
                action.changes_level = true;
                action.anchor = node.has_attr("href");
            }
            _ => {}
        }
    }
    action
}

struct Builder {
    document: WebDocument,
    counter: WordCounter,
    actions: Vec<Action>,
    tag_level: i32,
    group: i32,
    flush: bool,
    in_anchor: bool,
    text: String,
    nodes: Vec<NodeId>,
    first_word: Option<NodeId>,
    last_word: Option<NodeId>,
    block_level: i32,
    num_words: usize,
    linked_words: usize,
}

impl Builder {
    fn new(counter: WordCounter) -> Self {
        Self {
            document: WebDocument::default(),
            counter,
            actions: Vec::new(),
            tag_level: 0,
            group: 0,
            flush: false,
            in_anchor: false,
            text: String::new(),
            nodes: Vec::new(),
            first_word: None,
            last_word: None,
            block_level: -1,
            num_words: 0,
            linked_words: 0,
        }
    }

    fn flush_block(&mut self) {
        if self.nodes.is_empty() {
            return;
        }
        if let Some(first_word) = self.first_word.take() {
            let labels = self
                .actions
                .iter()
                .flat_map(|action| action.labels.iter().map(|value| (*value).to_owned()))
                .collect::<BTreeSet<_>>();
            let text_index = self.document.texts.len();
            let text = Text {
                text: std::mem::take(&mut self.text),
                nodes: std::mem::take(&mut self.nodes),
                first_word,
                last_word: self.last_word.unwrap(),
                labels,
                num_words: self.num_words,
                num_linked_words: self.linked_words,
                tag_level: self.block_level,
                group: self.group,
                element: self.document.elements.len(),
            };
            self.document.texts.push(text);
            self.document.elements.push(Element {
                kind: ElementKind::Text(text_index),
                is_content: false,
            });
        }
        self.text.clear();
        self.nodes.clear();
        self.last_word = None;
        self.block_level = -1;
        self.num_words = 0;
        self.linked_words = 0;
    }

    fn add_text(&mut self, document: &Document, index: NodeId, line_break: bool) {
        if self.flush {
            self.flush_block();
            self.group += 1;
            self.flush = false;
        }
        if line_break {
            self.text.push('\n');
            self.nodes.push(index);
            return;
        }
        let value = &document.nodes[index].data;
        if value.is_empty() {
            return;
        }
        self.text.push_str(value);
        self.nodes.push(index);
        if value.chars().all(char::is_whitespace) {
            return;
        }
        let words = self.counter.count(value);
        self.num_words += words;
        if self.in_anchor {
            self.linked_words += words;
        }
        self.first_word.get_or_insert(index);
        self.last_word = Some(index);
        if self.block_level == -1 {
            self.block_level = self.tag_level;
        }
    }

    fn start(&mut self, action: Action) {
        if action.changes_level {
            self.tag_level += 1;
        }
        if action.anchor {
            self.in_anchor = true;
            self.text.push(' ');
        }
        self.flush |= action.flush;
        self.actions.push(action);
    }

    fn end(&mut self) {
        let Some(action) = self.actions.last() else {
            return;
        };
        let (changes_level, flush, anchor) = (action.changes_level, action.flush, action.anchor);
        if changes_level {
            self.tag_level -= 1;
        }
        if self.flush || flush {
            self.flush_block();
            self.group += 1;
        }
        if anchor {
            self.in_anchor = false;
            self.text.push(' ');
        }
        self.actions.pop();
    }

    fn tag(&mut self, name: &str, start: bool) {
        self.flush_block();
        self.document.elements.push(Element {
            kind: ElementKind::Tag {
                name: name.into(),
                start,
            },
            is_content: false,
        });
    }
}

pub(crate) fn convert(
    document: &mut Document,
    root: NodeId,
    counter: WordCounter,
    skip_unlikelies: bool,
    page_url: Option<&str>,
) -> WebDocument {
    let mut builder = Builder::new(counter);
    builder.document.page_url = page_url.map(str::to_owned);
    walk(document, root, &mut builder, skip_unlikelies);
    builder.flush_block();
    builder.document
}

fn walk(document: &mut Document, index: NodeId, builder: &mut Builder, skip_unlikelies: bool) {
    if !visit(document, index, builder, skip_unlikelies) {
        return;
    }
    let mut next_child = document.nodes[index].children.first().copied();
    while let Some(child) = next_child {
        walk(document, child, builder, skip_unlikelies);
        next_child = document.nodes[child].parent.and_then(|parent| {
            let children = &document.nodes[parent].children;
            children
                .iter()
                .position(|&node| node == child)
                .and_then(|position| children.get(position + 1).copied())
        });
    }
    let tag = &document.nodes[index].tag;
    if can_be_nested(tag) {
        builder.tag(tag, false);
    }
    builder.end();
}

fn visit(
    document: &mut Document,
    index: NodeId,
    builder: &mut Builder,
    skip_unlikelies: bool,
) -> bool {
    static AUTHOR: LazyLock<Regex> =
        LazyLock::new(|| Regex::new("byline|author|dateline|writtenby|p-author").unwrap());
    static UNLIKELY: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new("-ad-|ai2html|banner|breadcrumbs|combx|comment|community|cover-wrap|disqus|extra|footer|gdpr|header|legends|menu|related|remark|replies|rss|shoutbox|sidebar|skyscraper|social|sponsor|supplemental|ad-break|agegate|pagination|pager|popup|yom-remote").unwrap()
    });
    static MAYBE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new("and|article|body|column|content|main|shadow").unwrap());
    let node = &document.nodes[index];
    match node.kind {
        Kind::Text => {
            builder.add_text(document, index, false);
            return false;
        }
        Kind::Element => {}
        _ => return false,
    }
    if !domutil::visible(document, index) {
        return false;
    }
    let class = node
        .attr("class")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if class == "sharing" || class == "socialArea" || node.attr("data-component") == "share" {
        return false;
    }
    let node_data = format!("{} {}", class, node.attr("id").trim()).to_ascii_lowercase();
    if node.attr("rel") == "author"
        || node.attr("itemprop").contains("author")
        || AUTHOR.is_match(&node_data)
    {
        let length = document.text(index).trim().chars().count();
        if length > 0 && length < 100 {
            return false;
        }
    }
    let tag = node.tag.clone();
    if skip_unlikelies {
        if UNLIKELY.is_match(&node_data)
            && !MAYBE.is_match(&node_data)
            && !document.has_ancestor(index, &["table"])
            && !matches!(tag.as_str(), "body" | "a")
        {
            return false;
        }
        if matches!(
            node.attr("role"),
            "menu"
                | "menubar"
                | "complementary"
                | "navigation"
                | "alert"
                | "alertdialog"
                | "dialog"
        ) {
            return false;
        }
    }
    if matches!(
        tag.as_str(),
        "div" | "section" | "header" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
    ) && !document.has_non_whitespace_text(index)
    {
        let children = node
            .children
            .iter()
            .filter(|&&child| document.nodes[child].kind == Kind::Element)
            .count();
        if children == 0
            || children
                == document
                    .elements(index)
                    .iter()
                    .filter(|&&child| matches!(document.nodes[child].tag.as_str(), "br" | "hr"))
                    .count()
        {
            return false;
        }
    }
    if let Some(image) = crate::embed::image(document, index)
        .or_else(|| crate::embed::site(document, index, builder.document.page_url.as_deref()))
    {
        builder.flush_block();
        builder.document.elements.push(Element {
            kind: image,
            is_content: false,
        });
        return false;
    }
    if can_be_nested(&tag) {
        builder.tag(&tag, true);
    }
    let node = &document.nodes[index];
    match tag.as_str() {
        "a" => {
            let href = node.attr("href");
            if href.contains("action=edit&section=") {
                return false;
            }
            if href.starts_with("javascript:")
                && node.children.len() == 1
                && document.nodes[node.children[0]].kind == Kind::Text
            {
                let child = node.children[0];
                if let Some(parent) = node.parent {
                    document.detach(child);
                    let position = document.nodes[parent]
                        .children
                        .iter()
                        .position(|&child| child == index)
                        .unwrap();
                    document.nodes[parent].children.insert(position, child);
                    document.nodes[child].parent = Some(parent);
                    document.detach(index);
                }
                builder.add_text(document, child, false);
                return false;
            }
        }
        "span" if class == "mw-editsection" => return false,
        "font" => {
            document.nodes[index].tag = "span".into();
            document.nodes[index].attrs.clear();
        }
        "br" => {
            builder.add_text(document, index, true);
            return false;
        }
        "table" if crate::table::classify(document, index).0 => {
            builder.flush_block();
            builder.document.elements.push(Element {
                kind: ElementKind::Table(index),
                is_content: false,
            });
            return false;
        }
        "video" => {
            builder.flush_block();
            builder.document.elements.push(Element {
                kind: ElementKind::Video(index),
                is_content: false,
            });
            return false;
        }
        "option" | "object" | "embed" | "applet" | "input" | "button" | "form" | "textarea"
        | "select" => {
            builder.flush = true;
            return false;
        }
        "head" | "style" | "script" | "link" | "noscript" | "iframe" | "svg" => return false,
        _ => {}
    }
    builder.start(action(document, index));
    true
}
