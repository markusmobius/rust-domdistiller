use crate::{
    dom::{Document, NodeId},
    domutil,
    stringutil::WordCounter,
};
use regex::Regex;
use std::{collections::BTreeMap, sync::LazyLock};

mod schema;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[cfg_attr(test, derive(serde::Deserialize))]
#[cfg_attr(test, serde(rename_all = "PascalCase"))]
pub struct MarkupArticle {
    pub published_time: String,
    pub modified_time: String,
    pub expiration_time: String,
    pub section: String,
    pub authors: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[cfg_attr(test, derive(serde::Deserialize))]
#[cfg_attr(test, serde(rename_all = "PascalCase"))]
pub struct MarkupImage {
    pub root: String,
    #[cfg_attr(test, serde(rename = "URL"))]
    pub url: String,
    #[cfg_attr(test, serde(rename = "SecureURL"))]
    pub secure_url: String,
    pub r#type: String,
    pub caption: String,
    pub width: i64,
    pub height: i64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[cfg_attr(test, derive(serde::Deserialize))]
#[cfg_attr(test, serde(rename_all = "PascalCase"))]
pub struct MarkupInfo {
    pub title: String,
    pub r#type: String,
    #[cfg_attr(test, serde(rename = "URL"))]
    pub url: String,
    pub description: String,
    pub publisher: String,
    pub copyright: String,
    pub author: String,
    pub article: MarkupArticle,
    pub images: Option<Vec<MarkupImage>>,
}

#[derive(Default)]
struct Accessor {
    info: MarkupInfo,
    has_article: bool,
    opt_out: bool,
}

fn lower(value: &str) -> String {
    value
        .chars()
        .map(|character| character.to_lowercase().next().unwrap())
        .collect()
}

fn integer(value: &str) -> i64 {
    match value.parse::<i64>() {
        Ok(value) => value,
        Err(error) => match error.kind() {
            std::num::IntErrorKind::PosOverflow => i64::MAX,
            std::num::IntErrorKind::NegOverflow => i64::MIN,
            _ => 0,
        },
    }
}

pub(crate) fn parse(document: &Document, root: NodeId) -> (MarkupInfo, String) {
    let accessors = [
        opengraph(document, root),
        schema::parse(document, root),
        ie_reader(document, root),
    ];
    let mut merged = MarkupInfo::default();
    let mut article_found = false;
    for accessor in &accessors {
        let info = &accessor.info;
        if merged.title.is_empty() {
            merged.title.clone_from(&info.title);
        }
        if merged.r#type.is_empty() {
            merged.r#type.clone_from(&info.r#type);
        }
        if merged.url.is_empty() {
            merged.url.clone_from(&info.url);
        }
        if merged.description.is_empty() {
            merged.description.clone_from(&info.description);
        }
        if merged.publisher.is_empty() {
            merged.publisher.clone_from(&info.publisher);
        }
        if merged.copyright.is_empty() {
            merged.copyright.clone_from(&info.copyright);
        }
        if merged.author.is_empty() {
            merged.author.clone_from(&info.author);
        }
        if !article_found && accessor.has_article {
            merged.article = info.article.clone();
            merged.article.authors.get_or_insert_default();
            article_found = true;
        }
        if merged.images.as_ref().is_none_or(Vec::is_empty) {
            merged.images.clone_from(&info.images);
        }
    }
    let title = merged.title.clone();
    if accessors.iter().any(|accessor| accessor.opt_out) {
        merged = MarkupInfo::default();
    }
    (merged, title)
}

fn ie_reader(document: &Document, root: NodeId) -> Accessor {
    let metas = document.tagged(root, "meta");
    let meta = |name: &str| {
        metas
            .iter()
            .find(|&&index| lower(document.nodes[index].attr("name")) == name)
            .map_or("", |&index| document.nodes[index].attr("content"))
    };
    let elements = document.elements(root);
    let by_class = |class: &str| {
        elements
            .iter()
            .find(|&&index| {
                document.nodes[index]
                    .attr("class")
                    .split_ascii_whitespace()
                    .any(|value| value == class)
            })
            .copied()
    };
    let author = by_class("byline-name")
        .map_or_else(String::new, |index| document.text(index).trim().into());
    let date = by_class("dateline").map_or_else(
        || meta("displaydate").into(),
        |index| document.text(index).trim().into(),
    );
    let publisher = elements
        .iter()
        .find_map(|&index| {
            let node = &document.nodes[index];
            [node.attr("publisher"), node.attr("source_organization")]
                .into_iter()
                .find(|value| !value.is_empty())
        })
        .unwrap_or("");
    let mut images = Vec::new();
    for index in document.tagged(root, "img") {
        let node = &document.nodes[index];
        let mut caption = String::new();
        if let Some(parent) = node
            .parent
            .filter(|&parent| document.nodes[parent].tag == "figure")
        {
            let captions = document.tagged(parent, "figcaption");
            if captions.len() <= 2 {
                for index in captions {
                    caption = domutil::inner_text(document, index);
                    if !caption.is_empty() {
                        break;
                    }
                }
            }
        }
        let width = node.attr("width").parse::<i64>();
        let height = node.attr("height").parse::<i64>();
        let relevant = match (&width, &height) {
            (Ok(width), Ok(height)) if *width >= 400 && *height > 0 => {
                (1.3..=3.0).contains(&(*width as f64 / *height as f64))
            }
            _ => false,
        };
        if !caption.is_empty() || relevant {
            images.push(MarkupImage {
                url: node.attr("src").into(),
                caption,
                width: integer(node.attr("width")),
                height: integer(node.attr("height")),
                ..MarkupImage::default()
            });
        }
    }
    Accessor {
        opt_out: lower(meta("ie_rm_off")) == "true",
        has_article: true,
        info: MarkupInfo {
            title: if document.tagged(root, "title").is_empty() {
                String::new()
            } else {
                meta("title").into()
            },
            publisher: publisher.into(),
            copyright: meta("copyright").into(),
            article: MarkupArticle {
                published_time: date,
                authors: if author.is_empty() {
                    None
                } else {
                    Some(vec![author.clone()])
                },
                ..MarkupArticle::default()
            },
            author,
            images: if images.is_empty() {
                None
            } else {
                Some(images)
            },
            ..MarkupInfo::default()
        },
    }
}

fn opengraph(document: &Document, root: NodeId) -> Accessor {
    static PREFIX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)(([A-Za-z0-9_]+):[\t\n\f\r ]+(http://ogp.me/ns(/[A-Za-z0-9_]+)*#))[\t\n\f\r ]*",
        )
        .unwrap()
    });
    static XML_NAME: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)^xmlns:([A-Za-z0-9_]+)").unwrap());
    static XML_VALUE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)^http://ogp.me/ns(/[A-Za-z0-9_]+)*#").unwrap());
    let mut prefixes = ["og".to_owned(), "profile".to_owned(), "article".to_owned()];
    let mut add_prefix =
        |prefix: &str, subtype: &str| match subtype.strip_prefix('/').unwrap_or(subtype) {
            "" => prefixes[0] = prefix.into(),
            "profile" => prefixes[1] = prefix.into(),
            "article" => prefixes[2] = prefix.into(),
            _ => {}
        };
    let mut prefix_value = if document.nodes[root].tag == "html" {
        document.nodes[root].attr("prefix")
    } else {
        ""
    };
    if prefix_value.is_empty() {
        prefix_value = document
            .tagged(root, "head")
            .first()
            .map_or("", |&head| document.nodes[head].attr("prefix"));
    }
    if !prefix_value.is_empty() {
        for capture in PREFIX.captures_iter(prefix_value) {
            add_prefix(
                &capture[2],
                capture.get(4).map_or("", |value| value.as_str()),
            );
        }
    } else {
        for (name, value) in &document.nodes[root].attrs {
            if let (Some(name), Some(value)) =
                (XML_NAME.captures(&lower(name)), XML_VALUE.captures(value))
            {
                add_prefix(&name[1], value.get(1).map_or("", |value| value.as_str()));
            }
        }
    }
    let properties = [
        ("title", 0, ""),
        ("type", 0, ""),
        ("url", 0, ""),
        ("description", 0, ""),
        ("site_name", 0, ""),
        ("image", 0, "image"),
        ("image:", 0, "image"),
        ("first_name", 1, "profile"),
        ("last_name", 1, "profile"),
        ("section", 2, "article"),
        ("published_time", 2, "article"),
        ("modified_time", 2, "article"),
        ("expiration_time", 2, "article"),
        ("author", 2, "article"),
    ];
    let mut values = BTreeMap::<&str, String>::new();
    let mut images = Vec::<MarkupImage>::new();
    let mut profile = None;
    let mut article_type = false;
    let mut authors = Vec::new();
    for index in document.tagged(root, "meta") {
        let node = &document.nodes[index];
        let original_property = node.attr("property");
        if !prefixes
            .iter()
            .any(|prefix| original_property.starts_with(prefix))
        {
            continue;
        }
        let mut property = lower(original_property);
        let content = node.attr("content");
        for &(name, prefix, kind) in &properties {
            let prefix = format!("{}:", prefixes[prefix]);
            if !property.starts_with(&format!("{prefix}{name}")) {
                continue;
            }
            property = property.strip_prefix(&prefix).unwrap().into();
            let mut add = true;
            match kind {
                "image" => {
                    if property == "image" {
                        images.push(MarkupImage {
                            root: content.into(),
                            ..MarkupImage::default()
                        });
                    } else if matches!(
                        property.as_str(),
                        "image:url"
                            | "image:secure_url"
                            | "image:type"
                            | "image:width"
                            | "image:height"
                    ) {
                        if images.is_empty() {
                            images.push(MarkupImage::default());
                        }
                        let image = images.last_mut().unwrap();
                        match property.as_str() {
                            "image:url" => image.url = content.into(),
                            "image:secure_url" => image.secure_url = content.into(),
                            "image:type" => image.r#type = content.into(),
                            "image:width" => image.width = integer(content),
                            "image:height" => image.height = integer(content),
                            _ => {}
                        }
                    }
                    add = false;
                }
                "profile" => {
                    add = *profile.get_or_insert_with(|| {
                        values
                            .get("type")
                            .is_some_and(|value| lower(value) == "profile")
                    });
                }
                "article" => {
                    if !article_type {
                        article_type = values
                            .get("type")
                            .is_some_and(|value| lower(value) == "article");
                    }
                    add = article_type;
                    if article_type && property == "author" {
                        authors.push(content.into());
                        add = false;
                    }
                }
                _ => {}
            }
            if add {
                values.insert(name, content.into());
            }
        }
    }
    images.retain(|image| !image.root.is_empty());
    for image in &mut images {
        if image.url.is_empty() {
            image.url.clone_from(&image.root);
        }
        image.root.clear();
    }
    let value = |name: &str| values.get(name).cloned().unwrap_or_default();
    if ["title", "type", "url"]
        .iter()
        .any(|name| value(name).is_empty())
        || images.is_empty()
    {
        return Accessor::default();
    }
    let mut author = String::new();
    if profile == Some(true) {
        author = value("first_name");
        let last = value("last_name");
        if !author.is_empty() && !last.is_empty() {
            author.push(' ');
            author.push_str(&last);
        }
    }
    let article = MarkupArticle {
        published_time: value("published_time"),
        modified_time: value("modified_time"),
        expiration_time: value("expiration_time"),
        section: value("section"),
        authors: if authors.is_empty() {
            None
        } else {
            Some(authors)
        },
    };
    Accessor {
        has_article: !article.published_time.is_empty()
            || !article.modified_time.is_empty()
            || !article.expiration_time.is_empty()
            || !article.section.is_empty()
            || article.authors.is_some(),
        info: MarkupInfo {
            title: value("title"),
            r#type: if lower(&value("type")) == "article" {
                "Article".into()
            } else {
                String::new()
            },
            url: value("url"),
            description: value("description"),
            publisher: value("site_name"),
            author,
            article,
            images: Some(images),
            ..MarkupInfo::default()
        },
        opt_out: false,
    }
}

pub(crate) fn document_title(document: &Document, root: NodeId, counter: WordCounter) -> String {
    static PATTERNS: LazyLock<[Regex; 5]> = LazyLock::new(|| {
        [
            r"(?i) [|\-\\/>\x{bb}] ",
            r"(?i) [\\/>\x{bb}] ",
            r"(?i)(.*)[|\-\\/>\x{bb}] .*",
            r"(?i)[^|\-\\/>\x{bb}]*[|\-\\/>\x{bb}](.*)",
            r"(?i)[|\-\\/>\x{bb}]+",
        ]
        .map(|pattern| Regex::new(pattern).unwrap())
    });
    let original = document
        .tagged(root, "title")
        .first()
        .map_or_else(String::new, |&node| domutil::inner_text(document, node));
    let mut current = original.clone();
    let mut hierarchical = false;
    if PATTERNS[0].is_match(&current) {
        hierarchical = PATTERNS[1].is_match(&current);
        current = PATTERNS[2].replace_all(&original, "$1").into_owned();
        if counter.count(&current) < 3 {
            current = PATTERNS[3].replace_all(&original, "$1").into_owned();
        }
    } else if current.contains(": ") {
        let matches_heading = document
            .tagged(root, "h1")
            .into_iter()
            .chain(document.tagged(root, "h2"))
            .any(|node| document.text(node).trim() == current.trim());
        if !matches_heading {
            current = original[original.rfind(':').unwrap() + 1..].into();
            if counter.count(&current) < 3 {
                current = original[original.find(':').unwrap() + 1..].into();
            } else if counter.count(&original[..original.find(':').unwrap()]) > 5 {
                current.clone_from(&original);
            }
        }
    } else if current.chars().count() > 150 || current.chars().count() < 15 {
        if let Some(&heading) = document.tagged(root, "h1").first() {
            current = domutil::inner_text(document, heading);
        }
    }
    current = current.split_whitespace().collect::<Vec<_>>().join(" ");
    let words = counter.count(&current);
    let original_without_separators = PATTERNS[4].replace_all(&original, "");
    if words <= 4
        && !original.is_empty()
        && (!hierarchical || words as i64 != counter.count(&original_without_separators) as i64 - 1)
    {
        current = original;
    }
    current
}
