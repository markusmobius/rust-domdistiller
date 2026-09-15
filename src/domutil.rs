use crate::dom::{Document, Kind, NodeId};
use regex::Regex;
use std::sync::LazyLock;

const ALLOWED_ATTRIBUTES: &str = "abbr accept-charset accept accesskey action align alink allow allowfullscreen allowpaymentrequest alt archive as async autocapitalize autocomplete autocorrect autofocus autoplay autopictureinpicture axis background behavior bgcolor border bordercolor capture cellpadding cellspacing char challenge charoff charset checked cite class classid clear code codebase codetype color cols colspan compact content contenteditable controls controlslist conversiondestination coords crossorigin csp data datetime declare decoding default defer dir direction dirname disabled disablepictureinpicture disableremoteplayback disallowdocumentaccess download draggable elementtiming enctype end enterkeyhint event exportparts face for form formaction formenctype formmethod formnovalidate formtarget frame frameborder headers height hidden high href hreflang hreftranslate hspace http-equiv id imagesizes imagesrcset importance impressiondata impressionexpiry incremental inert inputmode integrity is ismap keytype kind invisible label lang language latencyhint leftmargin link list loading longdesc loop low lowsrc manifest marginheight marginwidth max maxlength mayscript media method min minlength multiple muted name nohref nomodule nonce noresize noshade novalidate nowrap object open optimum part pattern placeholder playsinline ping policy poster preload pseudo readonly referrerpolicy rel reportingorigin required resources rev reversed role rows rowspan rules sandbox scheme scope scrollamount scrolldelay scrolling select selected shadowroot shadowrootdelegatesfocus shape size sizes slot span spellcheck src srcset srcdoc srclang standby start step style summary tabindex target text title topmargin translate truespeed trusttoken type usemap valign value valuetype version vlink vspace virtualkeyboardpolicy webkitdirectory width wrap";

pub(crate) fn strip_attributes(document: &mut Document, root: NodeId) {
    let indices = std::iter::once(root)
        .chain(document.elements(root))
        .collect::<Vec<_>>();
    for index in indices {
        let node = &mut document.nodes[index];
        let allow_size = matches!(node.tag.as_str(), "table" | "th" | "td" | "hr" | "pre");
        node.attrs.retain(|(name, _)| {
            !matches!(
                name.as_str(),
                "id" | "class"
                    | "align"
                    | "background"
                    | "bgcolor"
                    | "border"
                    | "cellpadding"
                    | "cellspacing"
                    | "frame"
                    | "hspace"
                    | "rules"
                    | "style"
                    | "valign"
                    | "vspace"
            ) && (allow_size || !matches!(name.as_str(), "width" | "height"))
                && ALLOWED_ATTRIBUTES
                    .split_ascii_whitespace()
                    .any(|allowed| allowed == name)
        });
    }
}

pub(crate) fn absolute_url(value: &str, base: Option<&str>) -> String {
    let Some(base) = base else {
        return value.into();
    };
    if value.is_empty()
        || value.starts_with('#')
        || value.starts_with("data:")
        || value.starts_with("javascript:")
    {
        return value.into();
    }
    if crate::urlutil::Url::request(value)
        .is_some_and(|url| !url.scheme.is_empty() && !url.hostname().is_empty())
    {
        return value.into();
    }
    match (
        crate::urlutil::Url::parse(base),
        crate::urlutil::Url::parse(value),
    ) {
        (Some(base), Some(reference)) => base.resolve(reference).to_string(),
        _ => value.into(),
    }
}

fn srcset_pattern() -> &'static Regex {
    static SRCSET: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)([^\t\n\f\r ]+)([\t\n\f\r ]+[0-9.]+[xw])?([\t\n\f\r ]*(?:,|$))").unwrap()
    });
    &SRCSET
}

pub(crate) fn srcset_urls(value: &str) -> Vec<String> {
    srcset_pattern()
        .captures_iter(value)
        .map(|capture| capture[1].to_owned())
        .collect()
}

pub(crate) fn make_absolute(
    document: &mut Document,
    root: NodeId,
    base: Option<&str>,
    links: bool,
) {
    let indices = std::iter::once(root)
        .chain(document.elements(root))
        .collect::<Vec<_>>();
    for index in indices {
        let node = &mut document.nodes[index];
        let attribute = match node.tag.as_str() {
            "a" if links => Some("href"),
            "img" | "source" | "track" | "video" => Some("src"),
            _ => None,
        };
        if let Some(attribute) = attribute {
            let value = node.attr(attribute);
            if !value.is_empty() {
                let value = absolute_url(value, base);
                node.set_attr(attribute, &value);
            }
        }
        if links && node.tag == "video" && !node.attr("poster").is_empty() {
            let value = absolute_url(node.attr("poster"), base);
            node.set_attr("poster", &value);
        }
        if node.has_attr("srcset") {
            let value = node.attr("srcset");
            if value.is_empty() {
                node.remove_attr("srcset");
            } else {
                let value = srcset_pattern()
                    .replace_all(value, |capture: &regex::Captures<'_>| {
                        format!(
                            "{}{}{}",
                            absolute_url(&capture[1], base),
                            capture.get(2).map_or("", |value| value.as_str()),
                            &capture[3]
                        )
                    })
                    .into_owned();
                node.set_attr("srcset", &value);
            }
        }
    }
}

pub(crate) fn processed_tree(
    source: &Document,
    root: NodeId,
    base: Option<&str>,
) -> Option<Document> {
    let selected = std::iter::once(root)
        .chain(source.descendants(root))
        .filter(|&node| matches!(source.nodes[node].kind, Kind::Element | Kind::Text))
        .collect::<Vec<_>>();
    let (mut copied, _) = clone_selected(source, &selected)?;
    make_absolute(&mut copied, 0, base, true);
    strip_attributes(&mut copied, 0);
    Some(copied)
}

pub(crate) fn clone_selected(source: &Document, selected: &[NodeId]) -> Option<(Document, NodeId)> {
    let mut ancestor = *selected.first()?;
    while !selected
        .iter()
        .all(|&node| source.contains(Some(ancestor), Some(node)))
    {
        ancestor = source.nodes[ancestor].parent?;
    }
    let mut included = std::collections::BTreeSet::new();
    for &node in selected {
        let mut current = Some(node);
        while let Some(index) = current {
            if !included.insert(index) || index == ancestor {
                break;
            }
            current = source.nodes[index].parent;
        }
    }
    let mut result = Document { nodes: Vec::new() };
    let mut pending = vec![(ancestor, None)];
    while let Some((original, parent)) = pending.pop() {
        let index = result.nodes.len();
        let mut node = source.nodes[original].clone();
        node.children.clear();
        node.parent = parent;
        result.nodes.push(node);
        if let Some(parent) = parent {
            result.nodes[parent].children.push(index);
        }
        pending.extend(
            source.nodes[original]
                .children
                .iter()
                .rev()
                .filter(|child| included.contains(child))
                .map(|&child| (child, Some(index))),
        );
    }
    Some((result, ancestor))
}

pub(crate) fn shallow_parent(
    target: &mut Document,
    root: NodeId,
    source: &Document,
    parent: NodeId,
) -> NodeId {
    let index = target.nodes.len();
    let mut node = source.nodes[parent].clone();
    node.parent = None;
    node.children.clear();
    target.nodes.push(node);
    target.append(index, root);
    index
}

pub(crate) fn text_output(
    source: &Document,
    text: &crate::webdoc::Text,
    text_only: bool,
) -> String {
    if text.labels.contains(crate::label::TITLE) {
        return String::new();
    }
    let Some((mut copied, mut original)) = clone_selected(source, &text.nodes) else {
        return String::new();
    };
    let mut root = 0;
    if copied.nodes[root].kind != Kind::Element {
        let Some(parent) = source.nodes[text.nodes[0]].parent else {
            return String::new();
        };
        root = shallow_parent(&mut copied, root, source, parent);
        original = parent;
    }
    if copied.nodes[root].tag == "body" {
        let fragment = Document::fragment(&copied.inner_html(root), "div");
        copied = fragment;
        root = copied.create_element("div");
        let fragment_root = copied.nodes[0].children[0];
        let children = copied.nodes[fragment_root].children.clone();
        for child in children {
            copied.append(root, child);
        }
    }
    while display(&copied, root) == "inline" {
        let Some(parent) = source.parent_element(original) else {
            break;
        };
        if source.nodes[parent].tag == "body" {
            break;
        }
        original = parent;
        root = shallow_parent(&mut copied, root, source, parent);
    }
    strip_attributes(&mut copied, root);
    if text_only {
        inner_text(&copied, root)
    } else if crate::webdoc::can_be_nested(&copied.nodes[root].tag) {
        copied.inner_html(root)
    } else {
        copied.outer_html(root)
    }
}

pub(crate) fn display(document: &Document, index: NodeId) -> &str {
    static DISPLAY: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)display:[\t\n\f\r ]*([A-Za-z0-9_-]+)[\t\n\f\r ]*(?:;|$)").unwrap()
    });
    let node = &document.nodes[index];
    let style = node.attr("style");
    if !style.is_empty() {
        if let Some(captures) = DISPLAY.captures(style) {
            return captures.get(1).unwrap().as_str();
        }
    }
    match node.tag.as_str() {
        "a" | "abbr" | "acronym" | "audio" | "b" | "bdi" | "bdo" | "br" | "canvas" | "circle"
        | "cite" | "code" | "data" | "defs" | "del" | "dfn" | "ellipse" | "em" | "embed"
        | "font" | "i" | "iframe" | "img" | "ins" | "kbd" | "label" | "lineargradient" | "mark"
        | "object" | "output" | "picture" | "polygon" | "q" | "rect" | "s" | "source" | "span"
        | "stop" | "strong" | "sub" | "sup" | "svg" | "tt" | "text" | "time" | "track" | "u"
        | "var" | "video" | "wbr" => "inline",
        "button" | "input" => "inline-block",
        "li" | "summary" => "list-item",
        "ruby" => "ruby",
        "rt" => "ruby-text",
        "table" => "table",
        "caption" => "table-caption",
        "td" | "th" => "table-cell",
        "col" => "table-column",
        "colgroup" => "table-column-group",
        "tfoot" => "table-footer-group",
        "thead" => "table-header-group",
        "tr" => "table-row",
        "tbody" => "table-row-group",
        "meta" | "script" | "style" | "link" => "none",
        _ => "block",
    }
}

pub(crate) fn visible(document: &Document, index: NodeId) -> bool {
    static HIDDEN: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)visibility:[\t\n\f\r ]*(:?hidden|collapse)").unwrap());
    let node = &document.nodes[index];
    display(document, index) != "none"
        && !node.has_attr("hidden")
        && !HIDDEN.is_match(node.attr("style"))
        && (node.attr("aria-hidden") != "true" || node.attr("class").contains("fallback-image"))
}

pub(crate) fn inner_text(document: &Document, root: NodeId) -> String {
    static PUNCTUATION: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"[\t\n\f\r ]+([.?!,;])[\t\n\f\r ]*([^\t\n\f\r ]*)").unwrap());
    static NEWLINE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"[\t\n\f\r ]*\|\\/\|[\t\n\f\r ]*").unwrap());
    let mut buffer = String::new();
    let mut pending = vec![root];
    while let Some(index) = pending.pop() {
        let node = &document.nodes[index];
        match node.kind {
            Kind::Text => {
                buffer.push(' ');
                buffer.push_str(&node.data);
                buffer.push(' ');
            }
            Kind::Element => {
                if node.tag == "br" {
                    buffer.push_str(r"|\/|");
                    continue;
                }
                if !visible(document, index) {
                    continue;
                }
            }
            _ => {}
        }
        pending.extend(node.children.iter().rev());
    }
    let normalized = buffer.split_whitespace().collect::<Vec<_>>().join(" ");
    let text = PUNCTUATION.replace_all(&normalized, "$1 $2");
    NEWLINE.replace_all(&text, "\n").into_owned()
}
