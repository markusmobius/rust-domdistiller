use super::{integer, lower, Accessor, MarkupArticle, MarkupImage, MarkupInfo};
use crate::dom::{Document, NodeId};
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Article,
    Image,
    Person,
    Organization,
}

struct Thing {
    kind: Kind,
    strings: BTreeMap<&'static str, String>,
    items: BTreeMap<&'static str, Option<usize>>,
}

impl Thing {
    fn new(kind: Kind) -> Self {
        let mut strings = vec!["name", "url", "description", "image"];
        let mut items = Vec::new();
        match kind {
            Kind::Article => {
                strings.extend([
                    "headline",
                    "publisher",
                    "copyrightHolder",
                    "copyrightYear",
                    "dateModified",
                    "datePublished",
                    "author",
                    "creator",
                    "articleSection",
                ]);
                items.extend([
                    "publisher",
                    "copyrightHolder",
                    "author",
                    "creator",
                    "associatedMedia",
                    "encoding",
                ]);
            }
            Kind::Image => strings.extend([
                "contentUrl",
                "encodingFormat",
                "caption",
                "representativeOfPage",
                "width",
                "height",
            ]),
            Kind::Person => strings.extend(["familyName", "givenName"]),
            Kind::Organization => strings.push("legalName"),
        }
        Self {
            kind,
            strings: strings
                .into_iter()
                .map(|name| (name, String::new()))
                .collect(),
            items: items.into_iter().map(|name| (name, None)).collect(),
        }
    }

    fn string(&self, name: &str) -> &str {
        self.strings.get(name).map_or("", String::as_str)
    }

    fn item(&self, name: &str) -> Option<usize> {
        self.items.get(name).copied().flatten()
    }

    fn person_or_organization(&self, name: &str, items: &[Thing]) -> String {
        let value = self.string(name);
        if !value.is_empty() {
            return value.into();
        }
        let Some(index) = self.item(name) else {
            return String::new();
        };
        let item = &items[index];
        if !matches!(item.kind, Kind::Person | Kind::Organization) {
            return String::new();
        }
        let name = item.string("name");
        if !name.is_empty() {
            return name.into();
        }
        if item.kind == Kind::Organization {
            return item.string("legalName").into();
        }
        let given = item.string("givenName");
        let family = item.string("familyName");
        format!(
            "{given}{}{family}",
            if !given.is_empty() && !family.is_empty() {
                " "
            } else {
                ""
            }
        )
    }

    fn image(&self) -> MarkupImage {
        let mut url = self.string("contentUrl");
        if url.is_empty() {
            url = self.string("url");
        }
        MarkupImage {
            url: url.into(),
            r#type: self.string("encodingFormat").into(),
            caption: self.string("caption").into(),
            width: integer(self.string("width")),
            height: integer(self.string("height")),
            ..MarkupImage::default()
        }
    }
}

fn item_kind(value: &str) -> Option<Kind> {
    match value.strip_prefix("http://schema.org/")? {
        "ImageObject" => Some(Kind::Image),
        "Article" | "BlogPosting" | "NewsArticle" | "ScholarlyArticle" | "TechArticle" => {
            Some(Kind::Article)
        }
        "Person" => Some(Kind::Person),
        "Organization"
        | "Corporation"
        | "EducationalOrganization"
        | "GovernmentOrganization"
        | "NGO" => Some(Kind::Organization),
        _ => None,
    }
}

fn property_value(document: &Document, index: NodeId) -> String {
    let node = &document.nodes[index];
    let attribute = match node.tag.as_str() {
        "img" | "audio" | "embed" | "iframe" | "source" | "track" | "video" => "src",
        "a" | "link" | "area" => "href",
        "meta" => "content",
        "time" => "datetime",
        "object" => "data",
        "data" | "meter" => "value",
        _ => "",
    };
    let value = node.attr(attribute);
    if value.is_empty() {
        document.text(index).trim().into()
    } else {
        value.into()
    }
}

pub(super) fn parse(document: &Document, root: NodeId) -> Accessor {
    let mut items = Vec::<Thing>::new();
    let mut item_elements = vec![None; document.nodes.len()];
    let is_scope = |index: NodeId| {
        document.nodes[index].has_attr("itemscope") && document.nodes[index].has_attr("itemtype")
    };
    for index in std::iter::once(root).chain(document.elements(root).into_iter().filter(|&index| {
        document.nodes[index].has_attr("itemprop") || document.nodes[index].has_attr("itemscope")
    })) {
        let node = &document.nodes[index];
        let mut parent_item = None;
        if index != root {
            let mut parent = node.parent;
            while let Some(parent_index) = parent {
                if is_scope(parent_index) {
                    parent_item = item_elements[parent_index];
                    break;
                }
                parent = document.nodes[parent_index].parent;
            }
        }
        let scope = is_scope(index);
        let new_item = if scope {
            item_kind(node.attr("itemtype")).map(|kind| {
                let item = items.len();
                items.push(Thing::new(kind));
                item_elements[index] = Some(item);
                item
            })
        } else {
            None
        };
        if let Some(parent) = parent_item {
            if scope && new_item.is_none() {
                continue;
            }
            for property in node.attr("itemprop").split_whitespace() {
                if let Some(item) = new_item {
                    if let Some(value) = items[parent].items.get_mut(property) {
                        value.get_or_insert(item);
                    }
                } else if let Some(value) = items[parent].strings.get_mut(property) {
                    if value.is_empty() {
                        *value = property_value(document, index).trim().into();
                    }
                }
            }
        }
    }
    let articles: Vec<_> = items
        .iter()
        .filter(|item| item.kind == Kind::Article)
        .collect();
    let mut info = MarkupInfo::default();
    for item in &articles {
        let mut title = item.string("headline");
        if title.is_empty() {
            title = item.string("name");
        }
        if !title.is_empty() {
            info.title = title.into();
            break;
        }
    }
    if let Some(&article) = articles.first() {
        info.r#type = "Article".into();
        info.url = article.string("url").into();
        info.description = article.string("description").into();
        info.publisher = article.person_or_organization("publisher", &items);
        if info.publisher.is_empty() {
            info.publisher = article.person_or_organization("copyrightHolder", &items);
        }
        let year = article.string("copyrightYear");
        let holder = article.person_or_organization("copyrightHolder", &items);
        if !year.is_empty() || !holder.is_empty() {
            info.copyright = format!(
                "Copyright {year}{}{holder}",
                if !year.is_empty() && !holder.is_empty() {
                    " "
                } else {
                    ""
                }
            );
        }
        info.author = article.person_or_organization("author", &items);
        if info.author.is_empty() {
            info.author = article.person_or_organization("creator", &items);
        }
        info.article = MarkupArticle {
            published_time: article.string("datePublished").into(),
            modified_time: article.string("dateModified").into(),
            section: article.string("articleSection").into(),
            authors: if info.author.is_empty() {
                None
            } else {
                Some(vec![info.author.clone()])
            },
            ..MarkupArticle::default()
        };
    }
    if info.author.is_empty() {
        for index in document.elements(root) {
            let node = &document.nodes[index];
            if matches!(node.tag.as_str(), "a" | "link") && node.attr("rel") == "author" {
                info.author = document.text(index).trim().into();
                if !info.author.is_empty() {
                    break;
                }
            }
        }
    }
    let mut images = Vec::new();
    let mut associated = None;
    for article in &articles {
        if associated.is_none() {
            associated = article
                .item("associatedMedia")
                .or_else(|| article.item("encoding"))
                .filter(|&index| items[index].kind == Kind::Image);
            if associated.is_some() {
                continue;
            }
        }
        let image = article.string("image");
        if !image.is_empty() {
            images.push(MarkupImage {
                url: image.into(),
                ..MarkupImage::default()
            });
        }
    }
    let mut representative = false;
    for (index, item) in items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.kind == Kind::Image)
    {
        let image = item.image();
        if associated == Some(index)
            || (!representative && lower(item.string("representativeOfPage")) == "true")
        {
            representative = true;
            images.insert(0, image);
        } else {
            images.push(image);
        }
    }
    if !images.is_empty() {
        info.images = Some(images);
    }
    Accessor {
        info,
        has_article: !articles.is_empty(),
        opt_out: false,
    }
}
