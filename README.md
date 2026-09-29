# rust-domdistiller

`rust-domdistiller` extracts article text, HTML, images, metadata and pagination
links from supplied HTML. It is a native Rust port of
[go-domdistiller](https://github.com/markusmobius/go-domdistiller), descended
from `chromium/dom-distiller` and `kohlschutter/boilerpipe`.

## Philosophy

Our extractor packages share three principles:

1. **Bring your own HTML.** Keep page acquisition separate from extraction.
   The primary workflow uses HTML supplied by the caller, who controls fetching,
   caching, rendering, retries and scheduling.
2. **Stay close to upstream.** Preserve the algorithms and behavior of each
   package's declared upstream reference as closely as possible. Document
   deliberate differences and compatibility limits in [UPSTREAM.md](UPSTREAM.md)
   rather than claiming exact equivalence on every page.
3. **Provide very fast Go and Rust packages.** Run extraction natively, without
   a Python or Java runtime. Improve throughput and allocation efficiency while
   preserving intended behavior, and substantiate performance with reproducible
   benchmarks that report quality alongside speed.

## Overview

The current `rust-domdistiller` release is **1.0.3**. It accepts HTML readers,
decoded strings, files or parsed trees and returns article HTML/text, title,
metadata, image URLs, word count and previous/next page links. It does not
fetch pages or run a browser. Callers own acquisition and concurrency.

Its behavioral reference is the `go-domdistiller` main-branch implementation
released as v1.0.0, not the separate stable branch or the original Java code.
This documentation-only release preserves the preceding release's runtime
source and dependency pins. No Go process or internal worker pool is required.

## Installation

```sh
cargo add rust-domdistiller@=1.0.3
```

Use Rust 1.98.1 or newer. Import the crate as `rust_domdistiller`. See
[Cargo.toml](Cargo.toml) for dependencies and [CHANGELOG.md](CHANGELOG.md)
for release changes.

## Usage

Extract text from HTML already held in memory:

```rust
use rust_domdistiller::{apply_for_reader, Options};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"<html><head><title>Research results</title></head><body><article>
<h1>Research results</h1>
<p>The research team compared several methods for extracting articles from saved
web pages. Every method received the same original HTML, and the evaluation
kept the reference text separate from the input supplied to each extractor.</p>
<p>The report records the complete experiment, including errors and repeated
measurements. Its results describe this collection of pages and do not promise
the same quality or execution time for every website.</p>
</article></body></html>"#;
    let options = Options {
        original_url: Some("https://example.org/research".into()),
        skip_pagination: true,
        ..Options::default()
    };
    let result = apply_for_reader(source.as_bytes(), &options)?;

    println!("{}", result.text);
    Ok(())
}
```

| Entry Point | Input |
| --- | --- |
| `apply` | A parsed `Document`, preserved during extraction |
| `apply_shared_document` | An integration wrapper implementing `AsRef<Document>` |
| `apply_to_node` | A selected subtree of a parsed document |
| `apply_for_reader` | Reader bytes, with charset detection and normalization |
| `apply_for_file` | A local file using the byte-reader pipeline |
| `apply_for_html` | Decoded UTF-8, without reader normalization |

Use `result.text` for text and `result.node.to_html()` for HTML. Results also
expose `title`, `markup_info`, `content_images`, `word_count`, `pagination_info`
and `timing_info`. See the
[rust-domdistiller API reference](https://docs.rs/rust-domdistiller/1.0.3/rust_domdistiller/)
for complete types and signatures.

## Options

Start with `Options::default()`:

| Option | Default | Effect |
| --- | --- | --- |
| `original_url` | `None` | Supplies URL context; `Some("")` remains distinct from no URL. |
| `skip_pagination` | `false` | Set to `true` to omit pagination detection. |
| `pagination_algo` | `PaginationAlgo::PrevNext` | Use scored previous/next links, or select `PageNumber` for numbered links. |

Pagination runs only with URL context and identifies links rather than
assembling a multi-page article. The comparison below disables it. URL
resolution follows the Go reference rather than WHATWG normalization.
`Document`, `Options` and `Result` are `Send + Sync`; independent calls may
share an immutable document while extraction makes its own working copies.

## Current Quality and Speed

The [2026-09-29 shared benchmark](https://github.com/markusmobius/content-extractor-benchmark/blob/ec719092d12f4d2a438dd29d9f4405aab6e0a321/README.md#results-2026-09-29)
compares the six packages below on **2,659 saved pages**: 983 LegoNews,
181 ScrapingHub and 1,495 WCXB.

### Extraction Speed

| Go Package (Measured Version) | Rust Package (Measured Version) | Go ms/page | Rust ms/page | Go/Rust |
| --- | --- | ---: | ---: | ---: |
| `go-readabilityV2` 0.6.0 | `rust-readability-v2` 0.6.5 | 4.755 | 3.945 | 1.21x |
| `go-domdistiller` 1.0.0 | `rust-domdistiller` 1.0.1 | 6.159 | 3.400 | 1.81x |
| `go-trafilatura` 2.2.6 (FAST) | `rust-trafilatura` 2.2.6 (FAST) | 11.329 | 6.570 | 1.72x |

Times are means of **all four measured passes after one warmup**. Go/Rust is
the named Go package's time divided by the named Rust package's time, not an
old/new release speedup. Later documentation-only releases do not change the
versions actually measured.

The run used Windows 11, Ryzen AI 7 PRO 350, Go 1.27.1 and Rust 1.98.1 GNU
with ThinLTO/mimalloc. Extraction includes required working copies, metadata
and text rendering. File I/O, startup, IPC, response serialization and scoring
are excluded. Comments and pagination are off; tables are on.
`go-trafilatura` and `rust-trafilatura` use FAST with external fallback disabled.
Power and sleep checks passed.

Parsing is separate: **Go 11.283 / Rust 6.386 ms/page**, charged once per
language/page for the shared suite. It includes decoding, DOM construction and
the separate `go-trafilatura` / `rust-trafilatura` noscript tree when needed.
These are extraction-stage comparisons, not complete request latencies.

### Text Quality

Each named pair has equal text scores. Errors are listed in LegoNews /
ScrapingHub / WCXB order and remain in the scoring denominators.

| Go Package | Rust Package | LegoNews F1 | ScrapingHub F1 | WCXB F1 | Errors |
| --- | --- | ---: | ---: | ---: | --- |
| `go-readabilityV2` | `rust-readability-v2` | 87.82711% | 95.20557% | 78.47603% | 7 / 0 / 28 |
| `go-domdistiller` | `rust-domdistiller` | 86.74080% | 92.74280% | 74.39696% | 0 / 0 / 0 |
| `go-trafilatura` (FAST) | `rust-trafilatura` (FAST) | 90.91534% | 96.15663% | 78.51703% | 4 / 0 / 10 |

The corpora use different scoring rules; their F1 scores must not be averaged.
Equal text scores do not imply identical metadata: `go-trafilatura` and
`rust-trafilatura` differ on one title and one author field. The
[full report](https://github.com/markusmobius/content-extractor-benchmark/blob/49c426d6135df81b7d492bea7e6aec8e6d77d80c/go_rust_shared_performance_2026_09_29.json)
contains metadata scores, differences, every pass and source/build identities.

## Compatibility and Limitations

- **Declared reference.** General `go-domdistiller` v1.0.0 behavior is the
  target. Deterministic differences are compatibility bugs, not acceptable
  merely because a finite test corpus misses them.
- **No browser rendering.** CSS-dependent browser behavior and JavaScript
  execution are not provided. The Go URL-fetching helper, detailed logging
  and parser timing entries are not ported.
- **Ambiguous ties.** Charset recognition and equally ranked numbered-page
  candidates can be nondeterministic in Go. Rust uses stable ordering while
  retaining the reference scores and ranking rules.
- **DOM invariants.** Callers editing node arenas must keep indices valid and
  acyclic. Attribute namespaces, order and duplicate local keys matter.
- **Not a sanitizer.** Sanitize extracted HTML before displaying untrusted input.

See [UPSTREAM.md](UPSTREAM.md) for source pins, compatibility boundaries,
the regression coverage ledger and historical measurements.

## Development

Use the pinned Rust toolchain. Ordinary tests need no Go runtime:

```sh
cargo fmt --all -- --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo package --locked
```

The optional live reference check requires Go 1.27.1 and cached or downloadable
pinned development dependencies:

```sh
cargo test --locked --lib live_go_parity -- --ignored --nocapture
```

Set `GO` to the executable if needed. Package verification requires a clean
release checkout. Documentation and release rules are in [AGENTS.md](AGENTS.md).

## License and Credits

The translation retains MIT, BSD-3-Clause, Apache-2.0 and ICU terms. See
[LICENSE](LICENSE), [NOTICE](NOTICE) and [licenses](licenses), including the
[complete Chromium notice](licenses/LICENSE-chromium.txt). Dependencies retain
their own licenses.

The Chromium Authors created `chromium/dom-distiller`, building on
Christian Kohlschuetter's `kohlschutter/boilerpipe`. Radhi Fadlillah implemented
the original `go-domdistiller` port and the inherited `go-shiori/dom` work.
Markus Mobius maintains `go-domdistiller` and this `rust-domdistiller` translation.
The Go Authors, chardet Authors and IBM/ICU contributors are also credited in the
retained notices for adapted URL, reader and charset behavior.