use crate::{
    converter,
    dom::{Document, Kind, NodeId},
    filter,
    markup::{self, MarkupInfo},
    pagination::{self, PaginationAlgo, PaginationInfo},
    stringutil::WordCounter,
};
use std::{
    fmt,
    io::{self, Read},
    path::Path,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Default)]
pub struct Options {
    pub original_url: Option<String>,
    pub skip_pagination: bool,
    pub pagination_algo: PaginationAlgo,
}

#[derive(Clone, Debug, Default)]
pub struct TimingEntry {
    pub name: String,
    pub time: Duration,
}

#[derive(Clone, Debug, Default)]
pub struct TimingInfo {
    pub markup_parsing_time: Duration,
    pub document_construction_time: Duration,
    pub article_processing_time: Duration,
    pub formatting_time: Duration,
    pub total_time: Duration,
    pub other_times: Vec<TimingEntry>,
}

#[derive(Clone, Debug)]
pub struct Result {
    pub url: String,
    pub title: String,
    pub markup_info: MarkupInfo,
    pub timing_info: TimingInfo,
    pub pagination_info: PaginationInfo,
    pub word_count: usize,
    pub node: Document,
    pub text: String,
    pub content_images: Vec<String>,
}

#[derive(Debug)]
pub enum Error {
    InvalidElement,
    CharsetNotDetected,
    InvalidUrl,
    OpenFile(io::Error),
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidElement => formatter.write_str("input doesn't have a valid element"),
            Self::CharsetNotDetected => formatter.write_str("Charset not detected."),
            Self::InvalidUrl => formatter.write_str("invalid original URL"),
            Self::OpenFile(error) => write!(formatter, "failed to open file: {error}"),
            Self::Io(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) | Self::OpenFile(error) => Some(error),
            Self::InvalidElement | Self::CharsetNotDetected | Self::InvalidUrl => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn apply_for_html(html: &str, options: &Options) -> std::result::Result<Result, Error> {
    apply(&Document::parse(html), options)
}

pub fn apply_for_reader(
    mut reader: impl Read,
    options: &Options,
) -> std::result::Result<Result, Error> {
    let mut input = Vec::new();
    reader.read_to_end(&mut input)?;
    let normalized = crate::encoding::decode(&input)?;
    apply_for_html(&normalized, options)
}

pub fn apply_for_file(
    path: impl AsRef<Path>,
    options: &Options,
) -> std::result::Result<Result, Error> {
    apply_for_reader(std::fs::File::open(path).map_err(Error::OpenFile)?, options)
}

pub fn apply(document: &Document, options: &Options) -> std::result::Result<Result, Error> {
    apply_to_node(document, 0, options)
}

pub fn apply_to_node(
    document: &Document,
    root: NodeId,
    options: &Options,
) -> std::result::Result<Result, Error> {
    let started = Instant::now();
    let Some(node) = document.nodes.get(root) else {
        return Err(Error::InvalidElement);
    };
    let root = if node.kind == Kind::Element {
        root
    } else {
        document
            .find_element(root, |_| true)
            .ok_or(Error::InvalidElement)?
    };
    let original_url = options
        .original_url
        .as_deref()
        .map(|value| {
            crate::urlutil::Url::parse(value)
                .map(|url| url.to_string())
                .ok_or(Error::InvalidUrl)
        })
        .transpose()?;
    let pagination_root = root;
    let root = document
        .find_element(root, |node| node.tag == "html")
        .unwrap_or(root);
    let mut timing = TimingInfo::default();
    let start = Instant::now();
    let (markup_info, markup_title) = markup::parse(document, root);
    timing.markup_parsing_time = start.elapsed();
    let counter = WordCounter::select(&document.text(root));
    let mut titles = Vec::new();
    if !markup_title.is_empty() {
        titles.push(markup_title);
    }
    titles.push(markup::document_title(document, root, counter));
    let start = Instant::now();
    let convert = |copied: &mut Document, skip_unlikelies| {
        let root = copied.replace_tree(document, root);
        let mut model = converter::convert(
            copied,
            root,
            counter,
            skip_unlikelies,
            original_url.as_deref(),
        );
        let mut blocks = model.create_text_blocks();
        filter::article(&mut blocks, &model, copied, counter, &titles);
        let words = blocks
            .iter()
            .filter(|block| block.is_content)
            .map(|block| block.num_words)
            .sum::<usize>();
        model.apply_blocks(&blocks);
        (model, words)
    };
    let mut copied = Document { nodes: Vec::new() };
    let (mut model, mut word_count) = convert(&mut copied, true);
    if word_count < 500 {
        (model, word_count) = convert(&mut copied, false);
    }
    timing.document_construction_time = start.elapsed();
    let start = Instant::now();
    model.retain_relevant_elements();
    model.retain_lead_image(&copied);
    model.retain_nested_elements();
    timing.article_processing_time = start.elapsed();
    let content_images = model.image_urls(&copied);
    let start = Instant::now();
    let text = model.output(&copied, true);
    let html = model.output(&copied, false);
    timing.formatting_time = start.elapsed();
    let parsed = Document::fragment(&html, "div");
    let mut node = Document { nodes: Vec::new() };
    let container = node.create_element("div");
    for &child in &parsed.nodes[parsed.nodes[0].children[0]].children {
        let child = node.import_tree(&parsed, child);
        node.append(container, child);
    }
    let mut pagination_info = PaginationInfo::default();
    if let Some(page_url) = original_url.as_deref().filter(|_| !options.skip_pagination) {
        let start = Instant::now();
        pagination_info = match options.pagination_algo {
            PaginationAlgo::PrevNext => {
                pagination::find_prev_next(document, pagination_root, page_url)
            }
            PaginationAlgo::PageNumber => {
                pagination::find_page_numbers(document, pagination_root, page_url, counter)
            }
        };
        timing.other_times.push(TimingEntry {
            name: "Pagination".into(),
            time: start.elapsed(),
        });
    }
    timing.total_time = started.elapsed();
    Ok(Result {
        url: original_url.unwrap_or_default(),
        title: titles.into_iter().next().unwrap_or_default(),
        markup_info,
        timing_info: timing,
        pagination_info,
        word_count,
        node,
        text,
        content_images,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{error::Error as _, io::Write};

    #[test]
    fn public_input_and_io_errors() {
        struct FailedReader;
        impl Read for FailedReader {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("reader failed"))
            }
        }
        let options = Options::default();
        let error = apply_for_reader(FailedReader, &options).unwrap_err();
        assert_eq!(error.to_string(), "reader failed");
        assert_eq!(error.source().unwrap().to_string(), "reader failed");
        assert!(matches!(
            apply(&Document { nodes: Vec::new() }, &options),
            Err(Error::InvalidElement)
        ));
        let document = Document::parse("<p>Article text.</p>");
        assert!(matches!(
            apply_to_node(&document, usize::MAX, &options),
            Err(Error::InvalidElement)
        ));
        let invalid = Options {
            original_url: Some("http://[invalid".into()),
            ..options.clone()
        };
        assert!(matches!(apply(&document, &invalid), Err(Error::InvalidUrl)));
        assert!(matches!(
            apply_for_reader([0xff].as_slice(), &options),
            Err(Error::CharsetNotDetected)
        ));
        assert!(apply_for_reader([].as_slice(), &options).is_ok());
        let directory = tempfile::tempdir().unwrap();
        let error = apply_for_file(directory.path().join("missing.html"), &options).unwrap_err();
        assert!(matches!(error, Error::OpenFile(_)));
        assert!(error.to_string().starts_with("failed to open file: "));
        let input = "<title>Cafe\u{301} title</title><p>co\u{ad}operate cafe\u{301}.</p>";
        let mut file = tempfile::NamedTempFile::new_in(directory.path()).unwrap();
        file.write_all(input.as_bytes()).unwrap();
        let from_file = apply_for_file(file.path(), &options).unwrap();
        let from_reader = apply_for_reader(input.as_bytes(), &options).unwrap();
        assert_eq!(from_file.title, "Caf\u{e9} title");
        assert_eq!(from_file.node, from_reader.node);
        assert_eq!(from_file.text, from_reader.text);
    }

    #[test]
    fn caller_owned_concurrency_preserves_input() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Document>();
        assert_send_sync::<Options>();
        assert_send_sync::<Result>();
        let html = format!("<title>Research news</title><article><p>{}</p><img src='photo.jpg'></article><nav><a href='?page=1'>Previous</a><a href='?page=3'>Next</a></nav>", "Detailed scientific article text. ".repeat(160));
        let document = Document::parse(&html);
        let original = document.clone();
        std::thread::scope(|scope| {
            for index in 0..4 {
                let document = &document;
                scope.spawn(move || {
                    let options = Options {
                        original_url: Some(format!("https://example.com/story{index}?page=2")),
                        skip_pagination: index % 2 == 0,
                        pagination_algo: if index < 2 {
                            PaginationAlgo::PrevNext
                        } else {
                            PaginationAlgo::PageNumber
                        },
                    };
                    let expected = apply(document, &options).unwrap();
                    for _ in 0..3 {
                        let actual = apply(document, &options).unwrap();
                        assert_eq!(actual.url, expected.url);
                        assert_eq!(actual.title, expected.title);
                        assert_eq!(actual.node, expected.node);
                        assert_eq!(actual.text, expected.text);
                        assert_eq!(actual.markup_info, expected.markup_info);
                        assert_eq!(actual.word_count, expected.word_count);
                        assert_eq!(actual.content_images, expected.content_images);
                        assert_eq!(actual.pagination_info, expected.pagination_info);
                        assert_eq!(
                            actual.timing_info.other_times.len(),
                            usize::from(!options.skip_pagination)
                        );
                    }
                });
            }
        });
        assert_eq!(document, original);
    }
}
