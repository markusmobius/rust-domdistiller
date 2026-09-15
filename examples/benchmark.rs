use rust_domdistiller::{apply, pagination::PaginationAlgo, Document, Options};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};
use std::time::Instant;

#[derive(Deserialize)]
struct Page {
    file: String,
    url: String,
    html: String,
    with: Vec<String>,
    without: Vec<String>,
}

#[derive(Default, Debug, PartialEq, Serialize)]
struct Counts {
    true_positives: usize,
    false_negatives: usize,
    false_positives: usize,
    true_negatives: usize,
}

impl Counts {
    fn evaluate(&mut self, text: &str, page: &Page) {
        for snippet in &page.with {
            if !text.is_empty() && text.contains(snippet) {
                self.true_positives += 1;
            } else {
                self.false_negatives += 1;
            }
        }
        for snippet in &page.without {
            if !text.is_empty() && text.contains(snippet) {
                self.false_positives += 1;
            } else {
                self.true_negatives += 1;
            }
        }
    }
}

#[derive(Deserialize)]
struct Request {
    pagination: String,
    #[serde(default)]
    outputs: bool,
}

#[derive(Serialize)]
struct Output {
    file: String,
    text: String,
    title: String,
    words: usize,
    html: String,
    next_page: String,
    prev_page: String,
}

#[derive(Serialize)]
struct Response {
    elapsed_ns: u128,
    counts: Counts,
    errors: Vec<String>,
    outputs: Vec<Output>,
}

fn options(request: &Request, page: &Page) -> Result<Options, String> {
    let pagination_algo = match request.pagination.as_str() {
        "skip" | "prev-next" => PaginationAlgo::PrevNext,
        "page-number" => PaginationAlgo::PageNumber,
        other => return Err(format!("unknown pagination mode: {other}")),
    };
    Ok(Options {
        original_url: Some(page.url.clone()),
        skip_pagination: request.pagination == "skip",
        pagination_algo,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("usage: benchmark CORPUS_JSON")?;
    let pages: Vec<Page> = serde_json::from_reader(io::BufReader::new(std::fs::File::open(path)?))?;
    let documents: Vec<Document> = pages
        .iter()
        .map(|page| Document::parse(&page.html))
        .collect();
    let stdout = io::stdout();
    let mut writer = io::BufWriter::new(stdout.lock());
    writeln!(writer, "{{\"ready\":{}}}", pages.len())?;
    writer.flush()?;
    for line in io::stdin().lock().lines() {
        let request: Request = serde_json::from_str(&line?)?;
        let configured = pages
            .iter()
            .map(|page| options(&request, page))
            .collect::<Result<Vec<_>, _>>()?;
        let mut counts = Counts::default();
        let mut errors = Vec::new();
        let mut outputs = Vec::new();
        let started = Instant::now();
        for ((page, document), options) in pages.iter().zip(&documents).zip(&configured) {
            match apply(document, options) {
                Ok(result) => {
                    counts.evaluate(&result.text, page);
                    if request.outputs {
                        outputs.push(Output {
                            file: page.file.clone(),
                            text: result.text,
                            title: result.title,
                            words: result.word_count,
                            html: result.node.to_html(),
                            next_page: result.pagination_info.next_page,
                            prev_page: result.pagination_info.prev_page,
                        });
                    }
                }
                Err(error) => {
                    counts.evaluate("", page);
                    errors.push(format!("{}: {error}", page.file));
                }
            }
        }
        let response = Response {
            elapsed_ns: started.elapsed().as_nanos(),
            counts,
            errors,
            outputs,
        };
        serde_json::to_writer(&mut writer, &response)?;
        writeln!(writer)?;
        writer.flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_snippet_scoring() {
        let page = Page {
            file: String::new(),
            url: String::new(),
            html: String::new(),
            with: vec!["article".into(), "missing".into(), "article".into()],
            without: vec!["navigation".into(), "footer".into()],
        };
        let mut counts = Counts::default();
        counts.evaluate("article navigation", &page);
        assert_eq!(
            counts,
            Counts {
                true_positives: 2,
                false_negatives: 1,
                false_positives: 1,
                true_negatives: 1,
            }
        );
        counts.evaluate("", &page);
        assert_eq!(counts.false_negatives, 4);
        assert_eq!(counts.true_negatives, 3);
    }
}
