use html5gum::emitters::html5ever::parse_document;
use markup5ever_rcdom::{NodeData, RcDom};

pub type NodeId = usize;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Kind {
    Document,
    Element,
    Text,
    Comment,
    Doctype,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Node {
    pub kind: Kind,
    pub tag: String,
    pub data: String,
    pub attrs: Vec<(String, String)>,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
}

impl Node {
    pub fn attr(&self, name: &str) -> &str {
        self.attrs
            .iter()
            .find(|(key, _)| key == name)
            .map_or("", |(_, value)| value)
    }

    pub fn has_attr(&self, name: &str) -> bool {
        self.attrs.iter().any(|(key, _)| key == name)
    }

    pub fn set_attr(&mut self, name: &str, value: &str) {
        if let Some((_, current)) = self.attrs.iter_mut().find(|(key, _)| key == name) {
            *current = value.into();
        } else {
            self.attrs.push((name.into(), value.into()));
        }
    }

    pub fn remove_attr(&mut self, name: &str) {
        if let Some(index) = self.attrs.iter().position(|(key, _)| key == name) {
            self.attrs.remove(index);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Document {
    pub nodes: Vec<Node>,
}

impl Document {
    pub fn parse(html: &str) -> Self {
        let parsed = parse_document(html, RcDom::default(), Default::default()).unwrap();
        Self::from_parsed(parsed)
    }

    pub(crate) fn fragment(html: &str, tag: &str) -> Self {
        let parsed = html5gum::emitters::html5ever::parse_fragment(
            html,
            RcDom::default(),
            Default::default(),
            html5ever::QualName::new(
                None,
                html5ever::Namespace::from("http://www.w3.org/1999/xhtml"),
                tag.into(),
            ),
            Vec::new(),
        )
        .unwrap();
        Self::from_parsed(parsed)
    }

    fn from_parsed(parsed: RcDom) -> Self {
        let mut nodes: Vec<Node> = Vec::new();
        let mut pending = vec![(parsed.document.clone(), None)];
        while let Some((handle, parent)) = pending.pop() {
            let mut node = Node {
                kind: Kind::Document,
                tag: String::new(),
                data: String::new(),
                attrs: Vec::new(),
                parent,
                children: Vec::new(),
            };
            let mut children = handle.children.borrow().clone();
            match &handle.data {
                NodeData::Document => {}
                NodeData::Element {
                    name,
                    attrs,
                    template_contents,
                    ..
                } => {
                    node.kind = Kind::Element;
                    node.tag = name.local.to_string();
                    node.attrs = attrs
                        .borrow()
                        .iter()
                        .map(|attribute| {
                            let key = attribute.name.prefix.as_ref().map_or_else(
                                || attribute.name.local.to_string(),
                                |prefix| format!("{prefix}:{}", attribute.name.local),
                            );
                            (key, attribute.value.to_string())
                        })
                        .collect();
                    if let Some(template) = template_contents.borrow().as_ref() {
                        children.extend(template.children.borrow().iter().cloned());
                    }
                }
                NodeData::Text { contents } => {
                    node.kind = Kind::Text;
                    node.data = contents.borrow().to_string();
                }
                NodeData::Comment { contents } => {
                    node.kind = Kind::Comment;
                    node.data = contents.to_string();
                }
                NodeData::Doctype {
                    name,
                    public_id,
                    system_id,
                } => {
                    node.kind = Kind::Doctype;
                    node.data = name.to_string();
                    if !public_id.is_empty() {
                        node.attrs.push(("public".into(), public_id.to_string()));
                    }
                    if !system_id.is_empty() {
                        node.attrs.push(("system".into(), system_id.to_string()));
                    }
                }
                NodeData::ProcessingInstruction { target, contents } => {
                    node.kind = Kind::Comment;
                    node.data = format!("?{target} {contents}?");
                }
            }
            let index = nodes.len();
            nodes.push(node);
            if let Some(parent) = parent {
                nodes[parent].children.push(index);
            }
            pending.extend(children.into_iter().rev().map(|child| (child, Some(index))));
        }
        Self { nodes }
    }

    pub fn elements(&self, root: NodeId) -> Vec<NodeId> {
        self.find_descendants(root, |node| node.kind == Kind::Element)
    }

    pub fn descendants(&self, root: NodeId) -> Vec<NodeId> {
        self.find_descendants(root, |_| true)
    }

    fn find_descendants(&self, root: NodeId, matches: impl Fn(&Node) -> bool) -> Vec<NodeId> {
        let mut result = Vec::new();
        let mut pending = self.nodes[root]
            .children
            .iter()
            .rev()
            .copied()
            .collect::<Vec<_>>();
        while let Some(index) = pending.pop() {
            let node = &self.nodes[index];
            if matches(node) {
                result.push(index);
            }
            pending.extend(node.children.iter().rev());
        }
        result
    }

    pub fn tagged(&self, root: NodeId, tag: &str) -> Vec<NodeId> {
        self.find_descendants(root, |node| node.kind == Kind::Element && node.tag == tag)
    }

    pub(crate) fn find_element(
        &self,
        root: NodeId,
        matches: impl Fn(&Node) -> bool,
    ) -> Option<NodeId> {
        let mut pending = self.nodes[root]
            .children
            .iter()
            .rev()
            .copied()
            .collect::<Vec<_>>();
        while let Some(index) = pending.pop() {
            let node = &self.nodes[index];
            if node.kind == Kind::Element && matches(node) {
                return Some(index);
            }
            pending.extend(node.children.iter().rev());
        }
        None
    }

    pub fn text(&self, root: NodeId) -> String {
        let mut output = String::new();
        let mut pending = vec![root];
        while let Some(index) = pending.pop() {
            if self.nodes[index].kind == Kind::Text {
                output.push_str(&self.nodes[index].data);
            }
            pending.extend(self.nodes[index].children.iter().rev());
        }
        output
    }

    pub(crate) fn has_non_whitespace_text(&self, root: NodeId) -> bool {
        let mut pending = vec![root];
        while let Some(index) = pending.pop() {
            let node = &self.nodes[index];
            if node.kind == Kind::Text && !node.data.trim().is_empty() {
                return true;
            }
            pending.extend(node.children.iter().rev());
        }
        false
    }

    pub fn parent_element(&self, index: NodeId) -> Option<NodeId> {
        self.nodes[index]
            .parent
            .filter(|&parent| self.nodes[parent].kind == Kind::Element)
    }

    pub fn has_ancestor(&self, index: NodeId, tags: &[&str]) -> bool {
        let mut current = self.parent_element(index);
        while let Some(parent) = current {
            if tags.contains(&self.nodes[parent].tag.as_str()) {
                return true;
            }
            current = self.parent_element(parent);
        }
        false
    }

    pub fn contains(&self, ancestor: Option<NodeId>, node: Option<NodeId>) -> bool {
        let (Some(ancestor), Some(mut current)) = (ancestor, node) else {
            return false;
        };
        loop {
            if ancestor == current {
                return true;
            }
            let Some(parent) = self.nodes[current].parent else {
                return false;
            };
            current = parent;
        }
    }

    pub fn detach(&mut self, index: NodeId) {
        if let Some(parent) = self.nodes[index].parent.take() {
            self.nodes[parent].children.retain(|&child| child != index);
        }
    }

    pub fn append(&mut self, parent: NodeId, child: NodeId) {
        self.detach(child);
        self.nodes[parent].children.push(child);
        self.nodes[child].parent = Some(parent);
    }

    pub fn create_element(&mut self, tag: &str) -> NodeId {
        let index = self.nodes.len();
        self.nodes.push(Node {
            kind: Kind::Element,
            tag: tag.into(),
            data: String::new(),
            attrs: Vec::new(),
            parent: None,
            children: Vec::new(),
        });
        index
    }

    pub(crate) fn create_text(&mut self, text: &str) -> NodeId {
        let index = self.nodes.len();
        self.nodes.push(Node {
            kind: Kind::Text,
            tag: String::new(),
            data: text.into(),
            attrs: Vec::new(),
            parent: None,
            children: Vec::new(),
        });
        index
    }

    pub(crate) fn import_tree(&mut self, source: &Self, root: NodeId) -> NodeId {
        let imported_root = self.nodes.len();
        self.copy_tree(source, root, imported_root);
        imported_root
    }

    pub(crate) fn replace_tree(&mut self, source: &Self, root: NodeId) -> NodeId {
        self.copy_tree(source, root, 0);
        0
    }

    fn copy_tree(&mut self, source: &Self, root: NodeId, start: usize) {
        if self.nodes.capacity() == 0 {
            self.nodes.reserve(source.descendants(root).len() + 1);
        }
        let mut pending = vec![(root, None)];
        let mut count = start;
        while let Some((original, parent)) = pending.pop() {
            let index = count;
            count += 1;
            let original = &source.nodes[original];
            if let Some(node) = self.nodes.get_mut(index) {
                node.kind = original.kind.clone();
                node.tag.clone_from(&original.tag);
                node.data.clone_from(&original.data);
                node.attrs.clone_from(&original.attrs);
                node.parent = parent;
                node.children.clear();
            } else {
                self.nodes.push(Node {
                    kind: original.kind.clone(),
                    tag: original.tag.clone(),
                    data: original.data.clone(),
                    attrs: original.attrs.clone(),
                    parent,
                    children: Vec::with_capacity(original.children.len()),
                });
            }
            if let Some(parent) = parent {
                self.nodes[parent].children.push(index);
            }
            pending.extend(
                original
                    .children
                    .iter()
                    .rev()
                    .map(|&child| (child, Some(index))),
            );
        }
        self.nodes.truncate(count);
    }

    pub fn to_html(&self) -> String {
        self.outer_html(0)
    }

    pub fn inner_html(&self, index: NodeId) -> String {
        self.render(
            self.nodes[index]
                .children
                .iter()
                .rev()
                .map(|&child| (child, false))
                .collect(),
        )
        .trim()
        .into()
    }

    pub fn outer_html(&self, index: NodeId) -> String {
        self.render(vec![(index, false)])
    }

    fn render(&self, mut pending: Vec<(NodeId, bool)>) -> String {
        let mut output = String::new();
        while let Some((index, closing)) = pending.pop() {
            let node = &self.nodes[index];
            if closing {
                output.push_str(&format!("</{}>", node.tag));
                continue;
            }
            match node.kind {
                Kind::Document => {}
                Kind::Element => {
                    output.push('<');
                    output.push_str(&node.tag);
                    for (key, value) in &node.attrs {
                        output.push(' ');
                        output.push_str(key);
                        output.push_str("=\"");
                        escape(value, &mut output);
                        output.push('"');
                    }
                    if matches!(
                        node.tag.as_str(),
                        "area"
                            | "base"
                            | "br"
                            | "col"
                            | "embed"
                            | "hr"
                            | "img"
                            | "input"
                            | "keygen"
                            | "link"
                            | "meta"
                            | "param"
                            | "source"
                            | "track"
                            | "wbr"
                    ) {
                        output.push_str("/>");
                        continue;
                    }
                    output.push('>');
                    if matches!(node.tag.as_str(), "pre" | "listing" | "textarea")
                        && node
                            .children
                            .first()
                            .is_some_and(|&child| self.nodes[child].data.starts_with('\n'))
                    {
                        output.push('\n');
                    }
                    if node.tag == "plaintext" {
                        output.push_str(&self.text(index));
                        return output;
                    }
                    pending.push((index, true));
                }
                Kind::Text => {
                    let raw = node.parent.is_some_and(|parent| {
                        matches!(
                            self.nodes[parent].tag.as_str(),
                            "script"
                                | "style"
                                | "xmp"
                                | "iframe"
                                | "noembed"
                                | "noframes"
                                | "noscript"
                        )
                    });
                    if raw {
                        output.push_str(&node.data);
                    } else {
                        escape(&node.data, &mut output);
                    }
                }
                Kind::Comment => {
                    output.push_str("<!--");
                    output.push_str(&node.data);
                    output.push_str("-->");
                }
                Kind::Doctype => {
                    output.push_str("<!DOCTYPE ");
                    output.push_str(&node.data);
                    if node.has_attr("public") {
                        output.push_str(" PUBLIC ");
                        quote_doctype(node.attr("public"), &mut output);
                        if node.has_attr("system") {
                            output.push(' ');
                            quote_doctype(node.attr("system"), &mut output);
                        }
                    } else if node.has_attr("system") {
                        output.push_str(" SYSTEM ");
                        quote_doctype(node.attr("system"), &mut output);
                    }
                    output.push('>');
                }
            }
            pending.extend(node.children.iter().rev().map(|&child| (child, false)));
        }
        output
    }
}

pub fn escape(text: &str, output: &mut String) {
    for character in text.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&#34;"),
            '\'' => output.push_str("&#39;"),
            '\r' => output.push_str("&#13;"),
            _ => output.push(character),
        }
    }
}

fn quote_doctype(text: &str, output: &mut String) {
    let quote = if text.contains('"') { '\'' } else { '"' };
    output.push(quote);
    output.push_str(text);
    output.push(quote);
}

#[cfg(test)]
mod tests {
    use super::{Document, Kind};

    #[test]
    fn replacement_restores_mutations_and_reuses_buffers() {
        let original = Document::parse("<article><p id='first'>Original text</p><figure><img src='first.jpg'><figcaption>Caption</figcaption></figure><p>Last text</p></article>");
        let original_root = original.tagged(0, "article")[0];
        let snapshot = original.clone();
        let mut copied = Document { nodes: Vec::new() };
        let root = copied.import_tree(&original, original_root);
        let capacity = copied.nodes.capacity();
        let text = copied
            .descendants(root)
            .into_iter()
            .find(|&index| copied.nodes[index].kind == Kind::Text)
            .unwrap();
        let text_buffer = copied.nodes[text].data.as_ptr();
        copied.nodes[text].data.clear();
        let paragraph = copied.tagged(root, "p")[0];
        copied.nodes[paragraph].tag = "span".into();
        copied.nodes[paragraph].attrs.clear();
        copied.detach(paragraph);
        let extra = copied.create_element("img");
        copied.append(root, extra);
        assert_eq!(copied.replace_tree(&original, original_root), 0);
        assert_eq!(copied.to_html(), original.outer_html(original_root));
        assert_eq!(copied.nodes[text].data.as_ptr(), text_buffer);
        assert!(copied.nodes.capacity() >= capacity);
        assert_eq!(
            copied.nodes.len(),
            original.descendants(original_root).len() + 1
        );
        assert!(copied.nodes[0].parent.is_none());
        for (parent, node) in copied.nodes.iter().enumerate() {
            for &child in &node.children {
                assert_eq!(copied.nodes[child].parent, Some(parent));
            }
        }
        let smaller = Document::parse("<p>Small</p>");
        let smaller_root = smaller.tagged(0, "p")[0];
        copied.replace_tree(&smaller, smaller_root);
        assert_eq!(copied.to_html(), "<p>Small</p>");
        assert_eq!(copied.nodes.len(), 2);
        let appended = copied.import_tree(&original, original_root);
        assert_eq!(appended, 2);
        assert_eq!(copied.outer_html(0), "<p>Small</p>");
        assert_eq!(
            copied.outer_html(appended),
            original.outer_html(original_root)
        );
        assert_eq!(original, snapshot);
    }

    #[test]
    fn filtered_traversals_preserve_preorder_and_whitespace() {
        for html in [
            "<div> <p><b>one</b></p><!--comment--><p>two</p></div>",
            "<div>\u{a0}<span>\u{2003}</span></div>",
            "<div><script>hidden text</script><span>\u{200b}</span></div>",
            "<div><br><hr></div>",
        ] {
            let document = Document::parse(html);
            for root in 0..document.nodes.len() {
                let descendants = document.descendants(root);
                let expected = descendants
                    .iter()
                    .copied()
                    .filter(|&index| document.nodes[index].kind == Kind::Element)
                    .collect::<Vec<_>>();
                assert_eq!(document.elements(root), expected);
                assert_eq!(
                    document.find_element(root, |_| true),
                    expected.first().copied()
                );
                for tag in ["div", "p", "span", "missing"] {
                    let expected = expected
                        .iter()
                        .copied()
                        .filter(|&index| document.nodes[index].tag == tag)
                        .collect::<Vec<_>>();
                    assert_eq!(document.tagged(root, tag), expected);
                    assert_eq!(
                        document.find_element(root, |node| node.tag == tag),
                        expected.first().copied()
                    );
                }
                assert_eq!(
                    document.has_non_whitespace_text(root),
                    !document.text(root).trim().is_empty()
                );
            }
        }
    }

    #[test]
    fn parsed_clone_preserves_original() {
        let original = Document::parse("<html><head><title>Example</title></head><body><p id='a'>One <b>two</b></p><img src='photo.png'></body></html>");
        let mut copied = original.clone();
        let paragraph = copied.tagged(0, "p")[0];
        copied.detach(paragraph);
        assert!(original
            .to_html()
            .contains("<p id=\"a\">One <b>two</b></p>"));
        assert!(!copied.to_html().contains("<p"));
        assert_eq!(original.text(paragraph), "One two");
        assert!(copied.to_html().contains("<img src=\"photo.png\"/>"));
    }
}
