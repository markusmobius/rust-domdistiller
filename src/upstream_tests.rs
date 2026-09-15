use crate::{
    filter, filter::classify_num_words, label, stringutil::WordCounter, webdoc::TextBlock,
};
use serde::Deserialize;

#[test]
fn original_go_document_title_tests() {
    let cases = [
        ("TitlelessRoot", "<div></div>", ""),
        ("TitledRoot", "<div><title>testing non-string document.title with a titled root</title></div>", "testing non-string document.title with a titled root"),
        ("MultiTitledRoot", "<div><title>first testing non-string document.title with a titled root</title><title>second testing non-string document.title with a titled root</title></div>", "first testing non-string document.title with a titled root"),
        ("1Dash2ShortParts", "<div><title>before dash - after dash</title></div>", "before dash - after dash"),
        ("1Dash2LongParts", "<div><title>part with 6 words before dash - part with 6 words after dash</title></div>", "part with 6 words before dash"),
        ("1Dash2LongPartsChinese", "<div><title>\u{6bd4}\u{8f03}\u{9577}\u{4e00}\u{9ede}\u{7684}\u{53e5}\u{5b50} - \u{9019}\u{662f}\u{4e0d}\u{8981}\u{7684}\u{90e8}\u{5206}</title></div>", "\u{6bd4}\u{8f03}\u{9577}\u{4e00}\u{9ede}\u{7684}\u{53e5}\u{5b50}"),
        ("1DashLongAndShortParts", "<div><title>part with 6 words before dash - after dash</title></div>", "part with 6 words before dash"),
        ("1DashShortAndLongParts", "<div><title>before dash - part with 6 words after dash</title></div>", "part with 6 words after dash"),
        ("1DashShortAndLongPartsChinese", "<div><title>\u{77ed}\u{8a9e} - \u{6bd4}\u{8f03}\u{9577}\u{4e00}\u{9ede}\u{7684}\u{53e5}\u{5b50}</title></div>", "\u{6bd4}\u{8f03}\u{9577}\u{4e00}\u{9ede}\u{7684}\u{53e5}\u{5b50}"),
        ("2DashesShortParts", "<div><title>before dash - between dash0 and dash1 - after dash1</title></div>", "before dash - between dash0 and dash1"),
        ("2DashesShortAndLongParts", "<div><title>before - - part with 6 words after dash</title></div>", "- part with 6 words after dash"),
        ("1Bar2ShortParts", "<div><title>before bar | after bar</title></div>", "before bar | after bar"),
        ("2ColonsShortParts", "<div><title>start : midway : end</title></div>", "start : midway : end"),
        ("2ColonsShortPartsChinese", "<div><title>\u{958b}\u{59cb} : \u{4e2d}\u{9593} : \u{6700}\u{5f8c}</title></div>", "\u{958b}\u{59cb} : \u{4e2d}\u{9593} : \u{6700}\u{5f8c}"),
        ("2ColonsShortAndLongParts", "<div><title>start : midway : part with 6 words at end</title></div>", "part with 6 words at end"),
        ("2ColonsShortAndLongPartsChinese", "<div><title>\u{958b}\u{59cb} : \u{4e2d}\u{9593} : \u{6700}\u{5f8c}\u{6bd4}\u{8f03}\u{9577}\u{7684}\u{90e8}\u{5206}</title></div>", "\u{6700}\u{5f8c}\u{6bd4}\u{8f03}\u{9577}\u{7684}\u{90e8}\u{5206}"),
        ("2ColonsShortAndLongAndShortParts", "<div><title>start : part with 6 words at midway : end</title></div>", "part with 6 words at midway : end"),
        ("2ColonsShortAndLongAndShortPartsChinese", "<div><title>\u{958b}\u{59cb} : \u{4e2d}\u{9593}\u{8981}\u{7684}\u{90e8}\u{5206} : \u{6700}\u{5f8c}</title></div>", "\u{4e2d}\u{9593}\u{8981}\u{7684}\u{90e8}\u{5206} : \u{6700}\u{5f8c}"),
        ("H1AsTitle", "<div><h1>long heading with 5 words</h1></div>", "long heading with 5 words"),
        ("MultiHeadingsWithLongText", "<div><h1>long heading1 with 5 words</h1><h2>long heading2 with 5 words</h2></div>", "long heading1 with 5 words"),
        ("H1WithLongHTML", "<div><h1><a href=\"http://longheading.com\"><b>long heading</b></a> with <br>5 words</h1></div>", "long heading with 5 words"),
        ("H1WithLongHTMLWithNbsp", "<div><h1><a href=\"http://longheading.com\"><b> &nbsp;long heading</b></a> with <br>5 words &nbsp; </h1></div>", "long heading with 5 words"),
    ];
    for (name, html, expected) in cases {
        let document = crate::Document::parse(html);
        let root = document.tagged(0, "div")[0];
        let counter = WordCounter::select(&document.text(root));
        assert_eq!(
            crate::markup::document_title(&document, root, counter),
            expected,
            "Test_Extractor_DocTitle_{name}"
        );
    }
}

#[test]
fn provenance_matches_retained_artifacts() {
    use sha2::{Digest, Sha256};
    #[derive(Deserialize)]
    struct SourceFile {
        destination: Option<String>,
        sha256: String,
        bytes: usize,
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let sources: Vec<SourceFile> =
        serde_json::from_str(include_str!("../testdata/provenance.json")).unwrap();
    assert_eq!(sources.len(), 21);
    for source in sources {
        if let Some(destination) = source.destination {
            let bytes = std::fs::read(root.join(&destination)).unwrap();
            assert_eq!(bytes.len(), source.bytes, "notice size: {destination}");
            assert_eq!(
                format!("{:x}", Sha256::digest(&bytes)),
                source.sha256,
                "notice hash: {destination}"
            );
        }
    }
    assert_eq!(
        format!(
            "{:x}",
            Sha256::digest(include_bytes!("../testdata/go-reference.json"))
        ),
        "4ae82b0de7db0c6459a138d8dbb3d139951bb3294d1318e5220c86b46fffc6e4"
    );
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../testdata/go-reference.json")).unwrap();
    for case in fixture["public"].as_array().unwrap() {
        let input = case["html"].as_str().unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(input.as_bytes())),
            case["sha256"].as_str().unwrap()
        );
    }
}

#[test]
#[ignore = "requires the pinned Go toolchain or GO_DOMDISTILLER_REFERENCE"]
fn live_go_parity() {
    let directory = tempfile::tempdir().unwrap();
    let fixture = if let Some(path) = std::env::var_os("GO_DOMDISTILLER_REFERENCE") {
        std::path::PathBuf::from(path)
    } else {
        let path = directory.path().join("go-reference.json");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let go = std::env::var_os("GO").unwrap_or_else(|| "go".into());
        let output = std::process::Command::new(go)
            .arg("-C")
            .arg(root.join("tools/go-reference"))
            .args(["run", "-mod=readonly", ".", "-output"])
            .arg(&path)
            .env("GOWORK", "off")
            .env("CGO_ENABLED", "0")
            .env("GOTOOLCHAIN", "go1.27.1")
            .output()
            .expect("run Go oracle; set GO or GO_DOMDISTILLER_REFERENCE");
        assert!(
            output.status.success(),
            "Go oracle failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        path
    };
    let actual = std::fs::read(fixture).unwrap();
    let expected = include_bytes!("../testdata/go-reference.json");
    let mismatch = actual
        .iter()
        .zip(expected)
        .position(|(left, right)| left != right);
    assert!(
        actual.len() == expected.len() && mismatch.is_none(),
        "live Go fixture differs: first mismatch {mismatch:?}, live {} bytes, saved {} bytes",
        actual.len(),
        expected.len()
    );
}

#[derive(Deserialize)]
struct Fixture {
    go_version: String,
    version: String,
    commit: String,
    words: Vec<WordCase>,
    classifiers: Vec<ClassifierCase>,
    filters: Vec<FilterCase>,
    dom: Vec<DomCase>,
    converters: Vec<ConverterCase>,
    siblings: Vec<SiblingCase>,
    tables: Vec<TableCase>,
    markup: Vec<MarkupCase>,
    extraction: Vec<ExtractionCase>,
    pagination: Vec<PaginationCase>,
    urls: Vec<UrlCase>,
    readers: Vec<ReaderCase>,
    encodings: Vec<EncodingCase>,
    decodings: Vec<DecodingCase>,
    public: Vec<PublicCase>,
}

#[derive(Deserialize)]
struct DecodingCase {
    charset: String,
    input: Vec<u8>,
    output: String,
}

#[test]
fn pinned_go_decoding_parity() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("../testdata/go-reference.json")).unwrap();
    assert_eq!(fixture.decodings.len(), 25245);
    let mut mismatches = std::collections::BTreeMap::<String, (usize, String)>::new();
    for (index, case) in fixture.decodings.into_iter().enumerate() {
        let actual = crate::encoding::decode_as(&case.input, &case.charset);
        if actual != case.output {
            let mismatch = mismatches.entry(case.charset).or_insert_with(|| {
                (
                    0,
                    format!(
                        "case {index}: {:?}, Rust {actual:?}, Go {:?}",
                        case.input, case.output
                    ),
                )
            });
            mismatch.0 += 1;
        }
    }
    assert!(mismatches.is_empty(), "decoder mismatches: {mismatches:#?}");
}

#[derive(Deserialize)]
struct PublicCase {
    name: String,
    html: String,
    input_url: String,
    skip: bool,
    number: bool,
    result: ExtractionCase,
    pagination: crate::pagination::PaginationInfo,
}

#[test]
fn pinned_go_public_parity() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("../testdata/go-reference.json")).unwrap();
    assert_eq!(fixture.public.len(), 168);
    for (index, case) in fixture.public.into_iter().enumerate() {
        let options = crate::Options {
            original_url: Some(case.input_url),
            skip_pagination: case.skip,
            pagination_algo: if case.number {
                crate::pagination::PaginationAlgo::PageNumber
            } else {
                crate::pagination::PaginationAlgo::PrevNext
            },
        };
        let result = crate::apply_for_reader(case.html.as_bytes(), &options).unwrap();
        assert_eq!(
            result.url, case.result.url,
            "URL case {index}: {}",
            case.name
        );
        assert_eq!(
            result.title, case.result.title,
            "title case {index}: {}",
            case.name
        );
        assert_eq!(
            result.markup_info, case.result.markup,
            "markup case {index}: {}",
            case.name
        );
        assert_eq!(
            result.word_count, case.result.words,
            "words case {index}: {}",
            case.name
        );
        assert_eq!(
            result.node.to_html(),
            case.result.node,
            "HTML case {index}: {}",
            case.name
        );
        assert_eq!(
            result.text, case.result.text,
            "text case {index}: {}",
            case.name
        );
        assert_eq!(
            result.content_images,
            case.result.images.unwrap_or_default(),
            "images case {index}: {}",
            case.name
        );
        assert_eq!(
            result.pagination_info, case.pagination,
            "pagination case {index}: {}",
            case.name
        );
    }
}

#[derive(Deserialize)]
struct EncodingCase {
    input: Vec<u8>,
    scores: std::collections::BTreeMap<String, i32>,
}

#[test]
fn pinned_go_encoding_scores() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("../testdata/go-reference.json")).unwrap();
    assert_eq!(fixture.encodings.len(), 571);
    for (index, case) in fixture.encodings.into_iter().enumerate() {
        let mut actual = std::collections::BTreeMap::new();
        for (charset, score) in crate::encoding::scores(&case.input) {
            if score > 0 {
                let current = actual.entry(charset.to_owned()).or_insert(0);
                *current = (*current).max(score);
            }
        }
        assert_eq!(
            actual,
            case.scores,
            "case {index}, {} bytes",
            case.input.len()
        );
    }
}

#[derive(Deserialize)]
struct ReaderCase {
    name: String,
    input: Vec<u8>,
    charsets: Vec<String>,
    parsed: String,
    error: String,
    title: String,
    node: String,
    text: String,
}

#[test]
fn pinned_go_reader_parity() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("../testdata/go-reference.json")).unwrap();
    assert_eq!(fixture.readers.len(), 43);
    for (index, case) in fixture.readers.into_iter().enumerate() {
        let result = crate::apply_for_reader(
            case.input.as_slice(),
            &crate::Options {
                skip_pagination: true,
                ..crate::Options::default()
            },
        );
        if !case.error.is_empty() {
            assert_eq!(
                result.unwrap_err().to_string(),
                case.error,
                "case {index}: {}",
                case.name
            );
            continue;
        }
        let result = result.unwrap();
        let decoded = crate::encoding::decode(&case.input).unwrap();
        assert_eq!(
            crate::Document::parse(&decoded).to_html(),
            case.parsed,
            "parsed case {index}: {} ({:?})",
            case.name,
            case.charsets
        );
        assert_eq!(
            result.title, case.title,
            "case {index}: {} ({:?})",
            case.name, case.charsets
        );
        assert_eq!(
            result.node.to_html(),
            case.node,
            "case {index}: {} ({:?})",
            case.name,
            case.charsets
        );
        assert_eq!(
            result.text, case.text,
            "case {index}: {} ({:?})",
            case.name, case.charsets
        );
    }
}

#[derive(Deserialize)]
struct UrlCase {
    base: Option<String>,
    input: String,
    absolute: String,
}

#[test]
fn pinned_go_url_parity() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("../testdata/go-reference.json")).unwrap();
    assert_eq!(fixture.urls.len(), 486);
    for (index, case) in fixture.urls.into_iter().enumerate() {
        assert_eq!(
            crate::domutil::absolute_url(&case.input, case.base.as_deref()),
            case.absolute,
            "case {index}: base {:?}, input {:?}",
            case.base,
            case.input
        );
    }
}

#[derive(Deserialize)]
struct PaginationCase {
    html: String,
    url: String,
    prev_next: crate::pagination::PaginationInfo,
    number: crate::pagination::PaginationInfo,
}

#[test]
fn pinned_go_pagination_parity() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("../testdata/go-reference.json")).unwrap();
    assert_eq!(fixture.pagination.len(), 1995);
    for (index, case) in fixture.pagination.into_iter().enumerate() {
        let document = crate::Document::parse(&case.html);
        let root = document.tagged(0, "html")[0];
        assert_eq!(
            crate::pagination::find_prev_next(&document, root, &case.url),
            case.prev_next,
            "case {index}: {} {}",
            case.url,
            case.html
        );
        assert_eq!(
            crate::pagination::find_outlink(&document, root, &case.url, false),
            case.prev_next.prev_page,
            "previous case {index}: {} {}",
            case.url,
            case.html
        );
        assert_eq!(
            crate::pagination::find_outlink(&document, root, &case.url, true),
            case.prev_next.next_page,
            "next case {index}: {} {}",
            case.url,
            case.html
        );
        assert_eq!(
            crate::pagination::find_page_numbers(
                &document,
                root,
                &case.url,
                WordCounter::select(&document.text(root))
            ),
            case.number,
            "number case {index}: {} {}",
            case.url,
            case.html
        );
    }
}

#[derive(Deserialize)]
struct ExtractionCase {
    html: String,
    url: String,
    root: String,
    title: String,
    markup: crate::markup::MarkupInfo,
    words: usize,
    node: String,
    text: String,
    images: Option<Vec<String>>,
}

#[test]
fn pinned_go_extraction_parity() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("../testdata/go-reference.json")).unwrap();
    assert_eq!(fixture.extraction.len(), 540);
    for (index, case) in fixture.extraction.into_iter().enumerate() {
        let document = crate::Document::parse(&case.html);
        let original = document.clone();
        let root = if case.root.is_empty() {
            0
        } else {
            document.tagged(0, &case.root)[0]
        };
        let options = crate::Options {
            original_url: if case.url.is_empty() {
                None
            } else {
                Some(case.url.clone())
            },
            skip_pagination: true,
            ..crate::Options::default()
        };
        let result = crate::apply_to_node(&document, root, &options).unwrap();
        assert_eq!(result.url, case.url, "case {index}");
        assert_eq!(result.title, case.title, "case {index}");
        assert_eq!(result.markup_info, case.markup, "case {index}");
        assert_eq!(result.word_count, case.words, "case {index}: {}", case.html);
        assert_eq!(
            result.node.to_html(),
            case.node,
            "case {index}: {}",
            case.html
        );
        assert_eq!(result.text, case.text, "case {index}");
        assert_eq!(
            result.content_images,
            case.images.unwrap_or_default(),
            "case {index}"
        );
        assert_eq!(document, original, "case {index}: caller document mutated");
        let repeated = crate::apply_to_node(&document, root, &options).unwrap();
        assert_eq!(repeated.node, result.node, "case {index}: repeated call");
    }
}

#[derive(Deserialize)]
struct MarkupCase {
    html: String,
    info: crate::markup::MarkupInfo,
    title: String,
    markup_title: String,
}

#[test]
fn pinned_go_markup_parity() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("../testdata/go-reference.json")).unwrap();
    assert_eq!(fixture.markup.len(), 335);
    for (index, case) in fixture.markup.into_iter().enumerate() {
        let document = crate::Document::parse(&case.html);
        let root = document.tagged(0, "html")[0];
        let (info, markup_title) = crate::markup::parse(&document, root);
        assert_eq!(info, case.info, "case {index}: {}", case.html);
        assert_eq!(markup_title, case.markup_title, "case {index}");
        let title = if markup_title.is_empty() {
            crate::markup::document_title(
                &document,
                root,
                WordCounter::select(&document.text(root)),
            )
        } else {
            markup_title
        };
        assert_eq!(title, case.title, "case {index}: {}", case.html);
    }
}

#[derive(Deserialize)]
struct TableCase {
    html: String,
    data: bool,
    reason: String,
}

#[test]
fn pinned_go_table_parity() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("../testdata/go-reference.json")).unwrap();
    assert_eq!(fixture.tables.len(), 2100);
    for (index, case) in fixture.tables.into_iter().enumerate() {
        let document = crate::Document::parse(&case.html);
        let table = document.tagged(0, "table")[0];
        assert_eq!(
            crate::table::classify(&document, table),
            (case.data, case.reason.as_str()),
            "case {index}: {}",
            case.html
        );
    }
}

#[derive(Deserialize)]
struct SiblingCase {
    html: String,
    input: Vec<BlockSnapshot>,
    output: Vec<BlockSnapshot>,
    largest: bool,
    cross_titles: bool,
    cross_headings: bool,
    mixed_tags: bool,
    max_density: f64,
    max_distance: usize,
    changed: bool,
}

#[test]
fn pinned_go_sibling_parity() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("../testdata/go-reference.json")).unwrap();
    assert_eq!(fixture.siblings.len(), 720);
    for (index, case) in fixture.siblings.into_iter().enumerate() {
        let mut document = crate::Document::parse(&case.html);
        let root = document.tagged(0, "html")[0];
        let model = crate::converter::convert(&mut document, root, WordCounter::Fast, false, None);
        let mut blocks: Vec<_> = case.input.into_iter().map(BlockSnapshot::block).collect();
        let changed = if case.largest {
            filter::keep_largest_with_siblings(&mut blocks, &model, &document)
        } else {
            filter::similar_siblings(
                &mut blocks,
                &model,
                &document,
                filter::SiblingOptions {
                    cross_titles: case.cross_titles,
                    cross_headings: case.cross_headings,
                    mixed_tags: case.mixed_tags,
                    max_link_density: case.max_density,
                    max_distance: case.max_distance,
                },
            )
        };
        assert_eq!(changed, case.changed, "case {index}");
        assert_eq!(
            blocks,
            case.output
                .into_iter()
                .map(BlockSnapshot::block)
                .collect::<Vec<_>>(),
            "case {index}: {}",
            case.html
        );
    }
}

#[derive(Deserialize)]
struct ConverterCase {
    input: String,
    skip: bool,
    elements: Vec<ElementSnapshot>,
    blocks: Vec<BlockSnapshot>,
    article: Vec<BlockSnapshot>,
    html: String,
    text: String,
    content: Vec<bool>,
    images: Vec<String>,
}

#[derive(Debug, Default, Deserialize, PartialEq)]
struct ElementSnapshot {
    kind: String,
    text: String,
    labels: Vec<String>,
    words: usize,
    anchor_words: usize,
    level: i32,
    group: i32,
    tag: String,
    start: bool,
}

#[test]
fn pinned_go_converter_parity() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("../testdata/go-reference.json")).unwrap();
    assert_eq!(fixture.converters.len(), 98);
    for case in fixture.converters {
        let mut document = crate::Document::parse(&case.input);
        let counter = WordCounter::select(&document.text(0));
        let root = document.tagged(0, "html")[0];
        let mut model = crate::converter::convert(
            &mut document,
            root,
            counter,
            case.skip,
            Some("https://example.com/news/story.html"),
        );
        let elements: Vec<_> = model
            .elements
            .iter()
            .map(|element| match &element.kind {
                crate::webdoc::ElementKind::Text(index) => {
                    let text = &model.texts[*index];
                    ElementSnapshot {
                        kind: "text".into(),
                        text: text.text.clone(),
                        labels: text.labels.iter().cloned().collect(),
                        words: text.num_words,
                        anchor_words: text.num_linked_words,
                        level: text.tag_level,
                        group: text.group,
                        ..ElementSnapshot::default()
                    }
                }
                crate::webdoc::ElementKind::Tag { name, start } => ElementSnapshot {
                    kind: "tag".into(),
                    tag: name.clone(),
                    start: *start,
                    ..ElementSnapshot::default()
                },
                crate::webdoc::ElementKind::Image { caption, .. } => ElementSnapshot {
                    kind: if caption.is_some() { "figure" } else { "image" }.into(),
                    ..ElementSnapshot::default()
                },
                crate::webdoc::ElementKind::Table(_) => ElementSnapshot {
                    kind: "table".into(),
                    ..ElementSnapshot::default()
                },
                crate::webdoc::ElementKind::Video(_) => ElementSnapshot {
                    kind: "video".into(),
                    ..ElementSnapshot::default()
                },
                crate::webdoc::ElementKind::Embed { .. } => ElementSnapshot {
                    kind: "embed".into(),
                    ..ElementSnapshot::default()
                },
            })
            .collect();
        assert_eq!(
            elements, case.elements,
            "elements (skip={}): {}",
            case.skip, case.input
        );
        let mut blocks = model.create_text_blocks();
        assert_eq!(
            blocks,
            case.blocks
                .into_iter()
                .map(BlockSnapshot::block)
                .collect::<Vec<_>>(),
            "blocks (skip={}): {}",
            case.skip,
            case.input
        );
        filter::article(&mut blocks, &model, &document, counter, &[]);
        assert_eq!(
            blocks,
            case.article
                .into_iter()
                .map(BlockSnapshot::block)
                .collect::<Vec<_>>(),
            "article (skip={}): {}",
            case.skip,
            case.input
        );
        model.apply_blocks(&blocks);
        model.retain_relevant_elements();
        model.retain_lead_image(&document);
        model.retain_nested_elements();
        assert_eq!(
            model
                .elements
                .iter()
                .map(|element| element.is_content)
                .collect::<Vec<_>>(),
            case.content,
            "content (skip={}): {}",
            case.skip,
            case.input
        );
        assert_eq!(
            model.output(&document, false),
            case.html,
            "HTML (skip={}): {}",
            case.skip,
            case.input
        );
        assert_eq!(
            model.output(&document, true),
            case.text,
            "text (skip={}): {}",
            case.skip,
            case.input
        );
        assert_eq!(
            model.image_urls(&document),
            case.images,
            "images (skip={}): {}",
            case.skip,
            case.input
        );
    }
}

#[derive(Deserialize)]
struct DomCase {
    input: String,
    html: String,
    text: String,
    inner_text: String,
    nodes: Vec<DomNode>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct DomNode {
    tag: String,
    display: String,
    visible: bool,
}

#[test]
fn pinned_go_dom_parity() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("../testdata/go-reference.json")).unwrap();
    assert_eq!(fixture.dom.len(), 41);
    for case in fixture.dom {
        let document = crate::Document::parse(&case.input);
        assert_eq!(document.to_html(), case.html, "HTML: {}", case.input);
        assert_eq!(document.text(0), case.text, "text: {}", case.input);
        assert_eq!(
            crate::domutil::inner_text(&document, 0),
            case.inner_text,
            "inner text: {}",
            case.input
        );
        let nodes: Vec<_> = document
            .elements(0)
            .into_iter()
            .map(|index| DomNode {
                tag: document.nodes[index].tag.clone(),
                display: crate::domutil::display(&document, index).into(),
                visible: crate::domutil::visible(&document, index),
            })
            .collect();
        assert_eq!(nodes, case.nodes, "nodes: {}", case.input);
    }
}

#[derive(Deserialize)]
struct WordCase {
    input: String,
    full: usize,
    letter: usize,
    fast: usize,
    selected: String,
}

#[derive(Deserialize)]
struct ClassifierCase {
    input: Vec<BlockInput>,
    content: Vec<bool>,
    changed: bool,
}

#[derive(Deserialize)]
struct BlockInput {
    words: usize,
    density: f64,
}

#[derive(Deserialize)]
struct FilterCase {
    name: String,
    input: Vec<BlockSnapshot>,
    output: Vec<BlockSnapshot>,
    titles: Vec<String>,
    changed: bool,
}

#[derive(Deserialize)]
struct BlockSnapshot {
    text: String,
    labels: Vec<String>,
    words: usize,
    anchor_words: usize,
    density: f64,
    level: i32,
    content: bool,
    elements: Vec<usize>,
}

impl BlockSnapshot {
    fn block(self) -> TextBlock {
        TextBlock {
            text: self.text,
            labels: self.labels.into_iter().collect(),
            num_words: self.words,
            num_words_in_anchor: self.anchor_words,
            link_density: self.density,
            tag_level: self.level,
            is_content: self.content,
            offset_start: self.elements.first().map_or(-1, |value| *value as i32),
            offset_end: self.elements.last().map_or(-1, |value| *value as i32),
            elements: self.elements,
        }
    }
}

#[test]
fn pinned_go_filter_parity() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("../testdata/go-reference.json")).unwrap();
    assert_eq!(fixture.filters.len(), 2880);
    for (index, case) in fixture.filters.into_iter().enumerate() {
        let mut blocks: Vec<_> = case.input.into_iter().map(BlockSnapshot::block).collect();
        let changed = match case.name.as_str() {
            "heading" => filter::heading_fusion(&mut blocks),
            "proximity-pre" => filter::block_proximity_fusion(&mut blocks, false),
            "proximity-post" => filter::block_proximity_fusion(&mut blocks, true),
            "largest" => filter::keep_largest_block(&mut blocks),
            "expand-title" => filter::expand_title_to_content(&mut blocks),
            "large-level" => filter::large_block_around_level(&mut blocks),
            "list" => filter::list_at_end(&mut blocks),
            "boilerplate-title" => filter::boilerplate_blocks(&mut blocks, label::TITLE),
            "boilerplate-empty" => filter::boilerplate_blocks(&mut blocks, ""),
            "label" => filter::label_to_boilerplate(&mut blocks, &[label::STRICTLY_NOT_CONTENT]),
            "terminating" => filter::terminating_blocks(&mut blocks),
            "title" => filter::document_title_match(&mut blocks, WordCounter::Fast, &case.titles),
            name => panic!("unknown filter {name}"),
        };
        assert_eq!(changed, case.changed, "case {index}: {}", case.name);
        assert_eq!(
            blocks,
            case.output
                .into_iter()
                .map(BlockSnapshot::block)
                .collect::<Vec<_>>(),
            "case {index}: {}",
            case.name
        );
    }
}

#[test]
fn pinned_go_word_and_classifier_parity() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("../testdata/go-reference.json")).unwrap();
    assert_eq!(fixture.go_version, "go1.27.1");
    assert_eq!(fixture.version, "v0.0.0-20240926050704-25b8d046ffb4");
    assert_eq!(fixture.commit, "25b8d046ffb4053bf68345d6fa59bc9ae1961ad8");
    for case in fixture.words {
        assert_eq!(
            WordCounter::Full.count(&case.input),
            case.full,
            "{:?}",
            case.input
        );
        assert_eq!(
            WordCounter::Letter.count(&case.input),
            case.letter,
            "{:?}",
            case.input
        );
        assert_eq!(
            WordCounter::Fast.count(&case.input),
            case.fast,
            "{:?}",
            case.input
        );
        let selected = match WordCounter::select(&case.input) {
            WordCounter::Full => "full",
            WordCounter::Letter => "letter",
            WordCounter::Fast => "fast",
        };
        assert_eq!(selected, case.selected, "{:?}", case.input);
    }
    for (index, case) in fixture.classifiers.into_iter().enumerate() {
        let mut blocks: Vec<_> = case
            .input
            .into_iter()
            .map(|input| TextBlock {
                num_words: input.words,
                link_density: input.density,
                ..TextBlock::default()
            })
            .collect();
        assert_eq!(classify_num_words(&mut blocks), case.changed, "{index}");
        assert_eq!(
            blocks
                .iter()
                .map(|block| block.is_content)
                .collect::<Vec<_>>(),
            case.content,
            "{index}"
        );
    }
}
