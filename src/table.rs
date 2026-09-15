use crate::{
    dom::{Document, NodeId},
    domutil,
};

pub(crate) fn classify(document: &Document, root: NodeId) -> (bool, &'static str) {
    let mut parent = document.nodes[root].parent;
    while let Some(index) = parent {
        let node = &document.nodes[index];
        if node.tag == "input" || node.attr("contenteditable").to_lowercase() == "true" {
            return (false, "InsideEditableArea");
        }
        parent = node.parent;
    }
    let table = &document.nodes[root];
    let role = table.attr("role").to_lowercase();
    if role == "presentation" {
        return (false, "RoleTable");
    }
    let landmark = |role: &str| {
        matches!(
            role,
            "application"
                | "banner"
                | "complementary"
                | "contentinfo"
                | "form"
                | "main"
                | "navigation"
                | "search"
        )
    };
    if landmark(&role) || matches!(role.as_str(), "grid" | "treegrid") {
        return (true, "RoleTable");
    }
    let descendants = document.elements(root);
    let nested = descendants
        .iter()
        .any(|&node| document.nodes[node].tag == "table");
    let direct: Vec<_> = descendants
        .into_iter()
        .filter(|&index| {
            if !nested {
                return true;
            }
            let mut parent = document.nodes[index].parent;
            while let Some(node) = parent {
                if document.nodes[node].tag == "table" {
                    return node == root;
                }
                parent = document.nodes[node].parent;
            }
            false
        })
        .collect();
    for &index in &direct {
        let role = document.nodes[index].attr("role").to_lowercase();
        if landmark(&role)
            || matches!(
                role.as_str(),
                "gridcell" | "columnheader" | "row" | "rowgroup" | "rowheader"
            )
        {
            return (true, "RoleDescendant");
        }
    }
    if table.attr("datatable") == "0" {
        return (false, "Datatable0");
    }
    if nested {
        return (false, "NestedTable");
    }
    let span = |value: &str| {
        let parsed = value.parse::<i64>().unwrap_or(0);
        if parsed == 0 {
            1
        } else {
            parsed
        }
    };
    let mut rows = 0i64;
    let mut columns = 0i64;
    for row in document.tagged(root, "tr") {
        rows = rows.wrapping_add(span(document.nodes[row].attr("rowspan")));
        let current = document
            .tagged(row, "td")
            .into_iter()
            .fold(0i64, |total, cell| {
                total.wrapping_add(span(document.nodes[cell].attr("colspan")))
            });
        columns = columns.max(current);
    }
    if rows <= 1 {
        return (false, "LessEq1Row");
    }
    if columns <= 1 {
        return (false, "LessEq1Col");
    }
    let valid_text = |node| !domutil::inner_text(document, node).trim().is_empty();
    let header = direct
        .iter()
        .find(|&&node| matches!(document.nodes[node].tag.as_str(), "colgroup" | "col" | "th"))
        .is_some_and(|&node| document.nodes[node].tag != "th" || valid_text(node));
    if document
        .tagged(root, "caption")
        .first()
        .is_some_and(|&node| valid_text(node))
        || !document.tagged(root, "thead").is_empty()
        || !document.tagged(root, "tfoot").is_empty()
        || header
    {
        return (true, "CaptionTheadTfootColgroupColTh");
    }
    let cells: Vec<_> = direct
        .iter()
        .copied()
        .filter(|&node| document.nodes[node].tag == "td")
        .collect();
    for &index in &cells {
        let node = &document.nodes[index];
        if ["abbr", "headers", "scope"]
            .iter()
            .any(|name| node.has_attr(name))
        {
            return (true, "AbbrHeadersScope");
        }
        let children = document.elements(index);
        if children.len() == 1 && document.nodes[children[0]].tag == "abbr" {
            return (true, "OnlyHasAbbr");
        }
    }
    if table.has_attr("summary") {
        return (true, "Summary");
    }
    if columns >= 5 {
        return (true, "MoreEq5Cols");
    }
    if rows >= 20 {
        return (true, "MoreEq20Rows");
    }
    if cells.len() <= 10 {
        return (false, "LessEq10Cells");
    }
    if direct.iter().any(|&node| {
        matches!(
            document.nodes[node].tag.as_str(),
            "embed" | "object" | "applet" | "iframe"
        )
    }) {
        return (false, "EmbedObjectAppletIframe");
    }
    (true, "Default")
}

pub(crate) fn output(
    document: &Document,
    root: NodeId,
    page_url: Option<&str>,
    text_only: bool,
) -> String {
    let Some(copied) = domutil::processed_tree(document, root, page_url) else {
        return String::new();
    };
    if text_only {
        domutil::inner_text(&copied, 0)
    } else {
        copied.to_html()
    }
}

pub(crate) fn image_urls(document: &Document, root: NodeId, page_url: Option<&str>) -> Vec<String> {
    let Some(copied) = domutil::processed_tree(document, root, page_url) else {
        return Vec::new();
    };
    let mut urls = Vec::new();
    for index in copied
        .elements(0)
        .into_iter()
        .filter(|&index| matches!(copied.nodes[index].tag.as_str(), "img" | "source"))
    {
        let source = copied.nodes[index].attr("src");
        if !source.is_empty() {
            urls.push(source.into());
        }
        urls.extend(domutil::srcset_urls(copied.nodes[index].attr("srcset")));
        for child in copied.elements(index) {
            urls.extend(domutil::srcset_urls(copied.nodes[child].attr("srcset")));
        }
    }
    urls
}
