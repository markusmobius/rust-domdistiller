# Rust-DomDistiller

Rust-DomDistiller finds the main readable content and metadata in an HTML page.
It is a native Rust port of [Go-DomDistiller](https://github.com/markusmobius/go-domdistiller),
which is based on Chromium's DOM Distiller and Boilerpipe. The pinned **Go
implementation is the behavioral reference**; there is no Python original.

The library extracts article HTML, text, title, metadata, images, and pagination
links from supplied documents. It requires no browser, Go process, Python runtime,
network access, or internal worker threads. Callers own fetching and concurrency.

## Usage

Add the crate to your project:

```toml
[dependencies]
rust-domdistiller = { git = "https://github.com/markusmobius/rust-domdistiller", tag = "v1.0.1" }
```

Version 1.0.1 is a GitHub source release, not a new crates.io publication.
See [CHANGELOG.md](CHANGELOG.md) for subsequent documentation updates.

```rust
use rust_domdistiller::{apply_for_reader, Options};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let html = "<title>Research update</title><article><p>The research team \
            published its results and described the evidence in detail.</p></article>";
let options = Options {
    original_url: Some("https://example.com/news/story".into()),
    skip_pagination: true,
    ..Options::default()
};
let result = apply_for_reader(html.as_bytes(), &options)?;
assert_eq!(result.title, "Research update");
println!("{}", result.node.to_html());
println!("{}", result.text);
# Ok(())
# }
```

The file example prints extracted HTML to stdout and its title and word count to
stderr:

```sh
cargo run --locked --release --example distill -- article.html https://example.com/news/story
```

Rust 1.98.1 is the minimum supported version and is pinned for repository builds.
Version 1.0.1 uses the Go revision documented below, also released as
Go-DomDistiller v1.0.0. The checked-in benchmark compares this extraction code
against that same Go implementation; the release version changes no algorithms.

## API

| Entry Point | Input Behavior |
| --- | --- |
| `apply(&Document, &Options)` | Extract from a parsed document without mutating it. |
| `apply_shared_document(&impl AsRef<Document>, &Options)` | Borrow a document from an integration wrapper; use the same extraction and private-copy behavior as `apply`. |
| `apply_to_node(&Document, NodeId, &Options)` | Extract from a selected subtree; preserve the caller's tree. |
| `apply_for_reader(impl Read, &Options)` | Detect charset, decode bytes, normalize NFD/remove soft hyphens/NFC, then extract, matching Go's reader pipeline. |
| `apply_for_file(path, &Options)` | Open the file and use the byte-reader pipeline. |
| `apply_for_html(&str, &Options)` | UTF-8 fast path: equivalent to extraction from `Document::parse`; no charset detection or reader normalization. |

`Options::default()` uses `pagination::PaginationAlgo::PrevNext`. Pagination runs
only when `original_url` is `Some` and `skip_pagination` is false. Set
`pagination_algo` to `PaginationAlgo::PageNumber` for Go's numbered-page algorithm.
URL parsing, resolution, raw host case, explicit ports, and URL serialization
follow the Go reference instead of WHATWG URL normalization. `Some("")` and `None`
are distinct, as an empty Go URL and a nil Go URL are distinct.

`Result` exposes `url`, `title`, `markup_info`, `pagination_info`, `word_count`,
`node`, `text`, `content_images`, and `timing_info`. The HTML result has a `div`
root. Metadata retains optional author/image lists to distinguish absent lists
from present empty lists. Durations measure the Rust execution; detailed Go
logging and parser timing entries are not implemented.

`Document`, `Options`, and `Result` are `Send + Sync`. Independent calls may share
an immutable document. DOM conversion uses a private copy, including the second
extraction attempt below Go's 500-word threshold. If modifying the public node
arena directly, keep its parent/child indices valid and acyclic.

`Document` implements `AsRef<Document>`. The shared-input entry point adds no
dependency on another extractor and does not replace the existing DOM types.

`Node.attrs` is `Vec<dom::Attribute>`, with `namespace`, `key`, and `value` fields,
replacing the earlier `(String, String)` tuples. Attribute helpers match Go's
first local-key occurrence; setting a value preserves its namespace, and removing
it exposes the next matching key. For example, an SVG `xlink:href` has namespace
`"xlink"` and key `"href"`, while an ordinary HTML `xlink:href` remains a literal
key. The `attr`, `has_attr`, `set_attr`, and `remove_attr` signatures are unchanged.

## Compatibility

The reference is Go-DomDistiller
`v0.0.0-20240926050704-25b8d046ffb4`, source
`25b8d046ffb4053bf68345d6fa59bc9ae1961ad8`, running on Go 1.27.1 with its pinned
dependencies. This follows that Go revision, not Chromium's original Java
implementation or Go-DomDistiller's separate stable branch.

**Go behavioral parity is the requirement, not just parity on tested cases.**
Deterministic differences are compatibility bugs, including on malformed HTML,
foreign content, unusual URLs, and inputs not yet covered by the suite. Tests
provide evidence of compatibility; they do not limit the supported input domain.

The offline corpus contains **44,528 Go-generated cases** covering word counters,
block classifiers and filters, DOM conversion, tables/media, metadata, both
pagination algorithms, URLs, charset scores, byte decoding, and full extraction.
It includes the upstream saved HTML page in all four pagination/skip settings.
Full results compare exact HTML, text, image order, metadata, title, word count,
and pagination. These are regression cases, not 44,528 independent web pages.

Parsing uses third-party html5gum and html5ever with a local Go-compatibility
adapter. Attribute namespaces, local keys, duplicates, and order are preserved
according to Go's rules. Go's extraction clones also omit element namespaces;
that is distinct from attribute namespaces, which both implementations retain.

Nondeterminism and API scope:

- Go's charset detector races equal-confidence recognizers. Rust uses a fixed
  recognition order with the same scores and creates no threads. A different
  winner is possible on genuinely ambiguous inputs. Saved reader cases assert
  that tied winners decode identically; non-equivalent ties are not waived.
- Go's numbered-pagination candidate map can break equal-ranked ties in map
  iteration order. Rust uses stable key ordering. Go itself has no fixed winner
  for those ties; candidate and ranking semantics must still match.
- There is no computed CSS, JavaScript execution, fetching, or browser layout,
  consistent with the server-side Go engine. Go's `ApplyForURL`, `LogFlags`,
  logging output, and detailed parser timing entries are not ported.
- The output follows Go's attribute and link handling. It is **not an HTML
  security sanitizer**; sanitize separately before displaying untrusted content.

See [UPSTREAM.md](UPSTREAM.md) for source pins, the coverage ledger, reproduction
commands, and retained attribution.

## Current Quality and Speed

The [2026-09-23 benchmark JSON](https://github.com/markusmobius/content-extractor-benchmark/blob/d433ab637f0a56c0926aa3698f470794a553472f/go_rust_shared_performance_2026_09_23.json)
is the source for these tables. All six released engines use the same 2,659
development pages: 983 LegoNews, 181 ScrapingHub and 1,495 WCXB. The three F1
scores use different scoring rules and must not be averaged. Errors are shown
in that corpus order and remain in the denominators.

| Implementation | LegoNews F1 | ScrapingHub F1 | WCXB F1 | Errors |
| --- | ---: | ---: | ---: | --- |
| go-readabilityV2-0.6.0 | 87.82711% | 95.20557% | 78.47603% | 7 / 0 / 28 |
| rust-readability-0.6.3 | 87.82711% | 95.20557% | 78.47603% | 7 / 0 / 28 |
| go-domdistiller-1.0.0 | 86.74080% | 92.74280% | 74.39696% | 0 / 0 / 0 |
| rust-domdistiller-1.0.1 | 86.74080% | 92.74280% | 74.39696% | 0 / 0 / 0 |
| go-trafilatura-2.2.2 | 90.88412% | 96.15663% | 78.49352% | 3 / 0 / 10 |
| rust-trafilatura-2.2.4 | 90.88412% | 96.15663% | 78.49352% | 3 / 0 / 10 |

| Implementation | Shared Parse ms/page | Extraction ms/page | Extraction ms/page, All Four Passes |
| --- | ---: | ---: | ---: |
| go-readabilityV2-0.6.0 | 5.638 | 2.669 | 2.705 |
| rust-readability-0.6.3 | 2.525 | 2.441 | 2.451 |
| go-domdistiller-1.0.0 | 5.638 | 3.618 | 3.628 |
| rust-domdistiller-1.0.1 | 2.525 | 1.965 | 1.973 |
| go-trafilatura-2.2.2 | 5.638 | 6.815 | 6.839 |
| rust-trafilatura-2.2.4 | 2.525 | 4.000 | 4.026 |

One full warmup precedes four measured passes. The first two timing columns
use the common best two complete passes (1 and 3), an optimistic estimate;
the final column retains the all-four mean. Go/Rust extraction ratios from
unrounded means are **1.09x Readability, 1.84x DomDistiller and 1.70x Trafilatura**.
Parsing is charged once per language/page, not once per engine. All-four parse
means are Go 5.667 and Rust 2.531 ms/page.

These Windows 11 / Ryzen AI 7 PRO 350 measurements use Go 1.27.1 and Rust
1.98.1 GNU, the released Trafilatura dependency graphs, and Rust ThinLTO/mimalloc.
Parsing includes eager decoding, normalization and DOM construction after the
file read; extraction includes private working copies, native metadata and text
rendering. Rust temporary trees are destroyed inside the timer; Go uses normal
GC, which can cross stage boundaries. File I/O, startup, IPC and scoring are
excluded. Fallbacks, comments and pagination are off; tables are on.

Every repeated scored output was stable. Go/Rust Readability and DomDistiller
match all scored outputs; Trafilatura retains two metadata-only differences.
Separate metadata scores, exact source pins and protocol limits are in
[UPSTREAM.md](UPSTREAM.md#released-suite-benchmark). DomDistiller remains 1.0.1;
these shared-input measurements use the suite's Readability parser, not the
standalone DomDistiller reader. Older measurements below use different protocols.

## Historical Patch Qualification

The [paired extraction benchmark](https://github.com/markusmobius/content-extractor-benchmark/blob/5edcfd090f1590c9bbf26d7543fbdc2ab615e117/rust_shared_performance_2026_09_21.json)
compares 1.0.0 with 1.0.1 in coordinated three-engine Rust suites on 2,659 pages,
with one full warmup and four paired passes. Parsing is separate and file reads
are untimed. DomDistiller takes 2.920 versus 2.917 ms/page on the same best two
passes (-0.11%); the all-four-pass difference is +0.43%. Both pass the 5%
regression gate. Scored text, metadata and errors match on every page.
These Windows GNU/Rust 1.98.1, ThinLTO/mimalloc measurements use the shared
Readability parser, with pagination off; they are not the standalone results below.

## Historical Quality and Speed

Both engines were evaluated on the same **983 labeled pages** from the pinned
[content-extractor-benchmark](https://github.com/markusmobius/content-extractor-benchmark/tree/466fdbee8a504441eb78ed11d71c1da220681cab).
These are the upstream benchmark's case-sensitive snippet scores, not token-level
scores or a claim of universal Go/Rust equivalence. All three pagination settings
produced the same counts: TP 2,535, FN 400, FP 375, TN 2,573.

| Engine | Precision | Recall | F1 |
| --- | ---: | ---: | ---: |
| Go-DomDistiller | 0.871 | 0.864 | 0.867 |
| Rust-DomDistiller | 0.871 | 0.864 | 0.867 |

Median elapsed time per complete 983-page pass, comparing Go with the corrected
Rust implementation on 2026-09-15:

| Pagination | Go | Rust | Go Time / Rust Time |
| --- | ---: | ---: | ---: |
| Skipped | 4,180 ms | 1,374 ms | 3.04x |
| Previous/next | 5,345 ms | 1,891 ms | 2.83x |
| Page number | 4,241 ms | 1,474 ms | 2.88x |

These measurements use Go 1.27.1 and Rust 1.98.1 release builds on an AMD Ryzen AI
7 PRO 350 under Linux/WSL2, pinned to one logical CPU. Each engine received two
warmup passes and ten measured passes per setting, in alternating engine order.
The timed work is extraction from pre-parsed DOMs plus snippet scoring. Input
I/O, charset decoding, initial parsing, IPC, and extra caller-side HTML
serialization are excluded. Extraction's own output generation remains included.
This is not an end-to-end file or network benchmark, and timings vary by machine.

All six compared fields match exactly on **983/983 pages in every setting**:
text, title, word count, HTML, next-page URL, and previous-page URL. The former SVG
attribute and archived-page pagination differences are fixed at their underlying
rules. The comparison runner fails on any output mismatch, even when snippet
scores agree. Metadata and image lists are checked separately by the differential
fixtures. Sample ranges, raw measurements, and reproduction commands are in
[UPSTREAM.md](UPSTREAM.md#corpus-benchmark).

## Verification

After Cargo dependencies are fetched, ordinary tests require neither Go nor
network access:

```sh
cargo fmt --all -- --check
cargo test --locked --offline
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --release
cargo build --locked --release
```

The opt-in live test requires Go 1.27.1 and downloads only the pinned development
dependencies when they are not cached:

```sh
cargo test --locked --lib live_go_parity -- --ignored --nocapture
```

Set `GO` to the Go executable if it is not on `PATH`. Alternatively, independently
export the fixture and set `GO_DOMDISTILLER_REFERENCE` to its absolute path. The
live check requires an exact byte match to the snapshot exercised by the offline
tests. Linux and Windows checks are configured in CI; unexecuted hosted jobs are
not evidence of platform support.

The original Go suite passes all packages (383 test functions in 47 files).
Rust also runs 22 directly translated upstream document-title scenarios, alongside
the 44,528 Go-generated regression cases. This is not a literal translation of
all 383 Go test functions. The separate labeled-page benchmark above reuses the
complete upstream manifest and scoring rules; its source-checkout runner is
[tools/benchmark.py](tools/benchmark.py).

## License

See [LICENSE](LICENSE), [NOTICE](NOTICE), and the retained files in
[licenses](licenses). The Rust translation retains the applicable Go-DomDistiller
MIT, Chromium/Go BSD, Boilerpipe Apache-2.0, and chardet/ICU notices. Cargo
dependencies retain their own licenses.