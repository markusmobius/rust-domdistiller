use crate::{
    dom::{Document, NodeId},
    domutil,
    webdoc::ElementKind,
};
use regex::Regex;
use std::sync::LazyLock;

fn root_domain(value: &str, root: &str) -> bool {
    let value = if value.starts_with("//") {
        format!("http:{value}")
    } else {
        value.into()
    };
    let Some(parsed) = crate::urlutil::Url::request(&value) else {
        return false;
    };
    parsed.host == root.as_bytes() || parsed.host.ends_with(format!(".{root}").as_bytes())
}

fn last_path(value: &str) -> String {
    crate::urlutil::Url::parse(value)
        .and_then(|url| {
            String::from_utf8_lossy(&url.path)
                .split('/')
                .rev()
                .map(str::trim)
                .find(|part| !part.is_empty())
                .map(str::to_owned)
        })
        .unwrap_or_default()
}

pub(crate) fn site(
    document: &Document,
    index: NodeId,
    page_url: Option<&str>,
) -> Option<ElementKind> {
    let node = &document.nodes[index];
    let make = |kind: &str, id: String| {
        if id.is_empty() {
            None
        } else {
            Some(ElementKind::Embed {
                node: index,
                kind: kind.into(),
                id,
            })
        }
    };
    if node.tag == "blockquote" {
        if !node.attr("class").contains("twitter-tweet") {
            return None;
        }
        let &anchor = document.tagged(index, "a").last()?;
        let href = domutil::absolute_url(document.nodes[anchor].attr("href"), page_url);
        return if root_domain(&href, "twitter.com") {
            make("twitter", last_path(&href))
        } else {
            None
        };
    }
    if node.tag == "iframe" {
        let source = node.attr("src");
        if root_domain(source, "twitter.com") && !node.attr("data-tweet-id").is_empty() {
            return make("twitter", node.attr("data-tweet-id").into());
        }
        let source = domutil::absolute_url(source, page_url);
        if root_domain(&source, "player.vimeo.com") {
            let id = last_path(&source);
            if id != "video" {
                return make("vimeo", id);
            }
        }
    }
    if !matches!(node.tag.as_str(), "iframe" | "object") {
        return None;
    }
    let mut source = node.attr("src").to_owned();
    if node.tag == "object" {
        if node.attr("type") == "application/x-shockwave-flash" {
            source = node.attr("data").into();
        } else if let Some(index) = document
            .tagged(index, "param")
            .into_iter()
            .find(|&node| document.nodes[node].attr("name") == "movie")
        {
            source = document.nodes[index].attr("value").into();
        }
    }
    if !source.contains('?') {
        source = source.replacen('&', "?", 1);
    }
    source = domutil::absolute_url(&source, page_url);
    if !root_domain(&source, "youtube.com") && !root_domain(&source, "youtube-nocookie.com") {
        return None;
    }
    let id = last_path(&source);
    if id == "embed" {
        None
    } else {
        make("youtube", id)
    }
}

pub(crate) fn site_output(document: &Document, index: NodeId, kind: &str, id: &str) -> String {
    let mut copied = Document { nodes: Vec::new() };
    let root = copied.create_element("div");
    copied.nodes[root].set_attr("class", "embed-placeholder");
    copied.nodes[root].set_attr("data-type", kind);
    copied.nodes[root].set_attr("data-id", id);
    if matches!(document.nodes[index].tag.as_str(), "blockquote" | "iframe") {
        let node = copied.import_tree(document, index);
        domutil::strip_attributes(&mut copied, node);
        copied.append(root, node);
    }
    copied.to_html()
}

pub(crate) fn video_output(document: &Document, root: NodeId, page_url: Option<&str>) -> String {
    let selected = std::iter::once(root)
        .chain(
            document.nodes[root]
                .children
                .iter()
                .copied()
                .filter(|&child| matches!(document.nodes[child].tag.as_str(), "source" | "track")),
        )
        .collect::<Vec<_>>();
    let (mut copied, _) = domutil::clone_selected(document, &selected).unwrap();
    let poster = copied.nodes[0].attr("poster");
    if !poster.is_empty() {
        let poster = domutil::absolute_url(poster, page_url);
        copied.nodes[0].set_attr("poster", &poster);
    }
    for index in std::iter::once(0).chain(copied.elements(0)) {
        let source = copied.nodes[index].attr("src");
        if !source.is_empty() {
            let source = domutil::absolute_url(source, page_url);
            copied.nodes[index].set_attr("src", &source);
        }
    }
    domutil::strip_attributes(&mut copied, 0);
    copied.to_html()
}

pub(crate) fn image(document: &mut Document, index: NodeId) -> Option<ElementKind> {
    match document.nodes[index].tag.as_str() {
        "figure" => {
            let mut found = None;
            let noscript = document.tagged(index, "noscript").first().copied();
            for tag in ["picture", "img"] {
                if let Some(noscript) = noscript {
                    found = document.tagged(noscript, "img").first().copied();
                    if found.is_none() {
                        let fragment = Document::fragment(&document.text(noscript), "div");
                        if let Some(&image) = fragment.tagged(0, tag).first() {
                            found = Some(document.import_tree(&fragment, image));
                        }
                    }
                    if let Some(image) = found {
                        document.detach(image);
                        document.nodes[index].children.insert(0, image);
                        document.nodes[image].parent = Some(index);
                    }
                }
                if found.is_none() {
                    found = document.tagged(index, tag).first().copied();
                }
                if found.is_some() {
                    break;
                }
            }
            let image = found?;
            if document.nodes[image].tag == "picture" {
                process_picture(document, image);
            }
            let caption = document.tagged(index, "figcaption").first().copied();
            let caption = match caption {
                Some(caption)
                    if document
                        .tagged(caption, "a")
                        .iter()
                        .any(|&node| document.nodes[node].has_attr("href")) =>
                {
                    caption
                }
                other => create_caption(document, other.unwrap_or(index)),
            };
            replace_lazy(document, image);
            Some(ElementKind::Image {
                node: image,
                caption: Some(caption),
            })
        }
        "span" => {
            if !document.nodes[index]
                .attr("class")
                .contains("lazy-image-placeholder")
            {
                return None;
            }
            let source = document.nodes[index].attr("data-src").to_owned();
            let srcset = document.nodes[index].attr("data-srcset").to_owned();
            let image = document.create_element("img");
            document.nodes[image].set_attr("src", &source);
            document.nodes[image].set_attr("srcset", &srcset);
            Some(ElementKind::Image {
                node: image,
                caption: None,
            })
        }
        "picture" => {
            process_picture(document, index);
            replace_lazy(document, index);
            Some(ElementKind::Image {
                node: index,
                caption: None,
            })
        }
        "img" => {
            replace_lazy(document, index);
            Some(ElementKind::Image {
                node: index,
                caption: None,
            })
        }
        _ => None,
    }
}

fn process_picture(document: &mut Document, index: NodeId) {
    for node in document.elements(index) {
        if !matches!(document.nodes[node].tag.as_str(), "img" | "source") {
            document.detach(node);
        }
    }
    if document.tagged(index, "img").is_empty() {
        if let Some(&source) = document.tagged(index, "source").first() {
            document.nodes[source].tag = "img".into();
        }
    }
}

fn create_caption(document: &mut Document, base: NodeId) -> NodeId {
    let fragment = Document::fragment(&domutil::inner_text(document, base), "div");
    let text = domutil::inner_text(&fragment, 0);
    let caption = document.create_element("figcaption");
    let text = document.create_text(text.trim());
    document.append(caption, text);
    caption
}

fn replace_lazy(document: &mut Document, root: NodeId) {
    static DATA: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)^data:[\t\n\f\r ]*([^\t\n\f\r ;,]+)[\t\n\f\r ]*;[\t\n\f\r ]*base64[\t\n\f\r ]*",
        )
        .unwrap()
    });
    static SOURCE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^[\t\n\f\r ]*[^\t\n\f\r ]+\.(jpg|jpeg|png|webp)[^\t\n\f\r ]*[\t\n\f\r ]*$")
            .unwrap()
    });
    static SRCSET: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)\.(jpg|jpeg|png|webp)[\t\n\f\r ]+[0-9]").unwrap());
    let mut nodes = document
        .elements(root)
        .into_iter()
        .filter(|&index| matches!(document.nodes[index].tag.as_str(), "img" | "source"))
        .collect::<Vec<_>>();
    nodes.push(root);
    for index in nodes {
        let node = &mut document.nodes[index];
        let mut source = node.attr("src").to_owned();
        if !source.is_empty()
            && DATA.captures(&source).is_some_and(|capture| {
                &capture[1] != "image/svg+xml"
                    && source.len() as i64
                        - (source.find("base64").map_or(-1, |position| position as i64) + 7)
                        < 133
            })
        {
            node.remove_attr("src");
            source.clear();
        }
        if let Some(value) = ["data-src", "data-original", "datasrc", "data-url"]
            .iter()
            .map(|name| node.attr(name))
            .find(|value| !value.is_empty())
        {
            source = value.into();
        }
        if source.is_empty() {
            if let Some((_, value)) = node.attrs.iter().find(|(_, value)| SOURCE.is_match(value)) {
                source = value.clone();
            }
        }
        if !source.is_empty() {
            node.set_attr("src", &source);
        }
        if node.attr("src").is_empty() {
            let mut srcset = node.attr("srcset").to_owned();
            if let Some(value) = ["data-srcset", "datasrcset"]
                .iter()
                .map(|name| node.attr(name))
                .find(|value| !value.is_empty())
            {
                srcset = value.into();
            }
            if srcset.is_empty() {
                if let Some((_, value)) =
                    node.attrs.iter().find(|(_, value)| SRCSET.is_match(value))
                {
                    srcset = value.clone();
                }
            }
            if !srcset.is_empty() {
                node.set_attr("srcset", &srcset);
            }
        }
    }
}

fn processed_image(document: &Document, node: NodeId, page_url: Option<&str>) -> Document {
    let mut image = Document { nodes: Vec::new() };
    image.import_tree(document, node);
    domutil::make_absolute(&mut image, 0, page_url, false);
    domutil::strip_attributes(&mut image, 0);
    image
}

pub(crate) fn image_urls(document: &Document, node: NodeId, page_url: Option<&str>) -> Vec<String> {
    let image = processed_image(document, node, page_url);
    let mut urls = Vec::new();
    let source = image.nodes[0].attr("src");
    if !source.is_empty() {
        urls.push(source.into());
    }
    urls.extend(domutil::srcset_urls(image.nodes[0].attr("srcset")));
    for index in image.elements(0) {
        urls.extend(domutil::srcset_urls(image.nodes[index].attr("srcset")));
    }
    urls
}

pub(crate) fn image_output(
    document: &Document,
    node: NodeId,
    caption: Option<NodeId>,
    page_url: Option<&str>,
    text_only: bool,
) -> String {
    let Some(caption) = caption else {
        return if text_only {
            String::new()
        } else {
            processed_image(document, node, page_url).to_html()
        };
    };
    let caption_tree = domutil::processed_tree(document, caption, page_url).unwrap();
    if text_only {
        return domutil::inner_text(&caption_tree, 0);
    }
    let mut image = processed_image(document, node, page_url);
    let figure = image.create_element("figure");
    image.append(figure, 0);
    if !document.inner_html(caption).is_empty() {
        let caption = image.import_tree(&caption_tree, 0);
        image.append(figure, caption);
    }
    domutil::strip_attributes(&mut image, figure);
    image.outer_html(figure)
}
