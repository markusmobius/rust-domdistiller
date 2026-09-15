# Upstream and Compatibility Ledger

## Authority

Go-DomDistiller is the source of truth. There is no Python reference for this
component. The port retains Go's actual thresholds, ordering, retry decisions,
metadata precedence, pagination rules, and output behavior, including observed
quirks. It does not substitute a different readability algorithm.

Behavioral parity is the requirement for supplied documents, not a property
promised only for fixture inputs. Any deterministic Go/Rust output difference is
a compatibility bug, including on malformed HTML, foreign content, unusual URLs,
and inputs not yet represented in the tests. The checks below record verification
coverage; they do not define a smaller compatibility contract.

| Source | Pin |
| --- | --- |
| Go module | `github.com/markusmobius/go-domdistiller` |
| Module version | `v0.0.0-20240926050704-25b8d046ffb4` |
| Source commit | `25b8d046ffb4053bf68345d6fa59bc9ae1961ad8` |
| Module checksum | `h1:+7kfF1+dmSXV469sqjeNC+eKJF7xDuS5mvZA3DFVLLY=` |
| go.mod checksum | `h1:E7PoeC3nd4GqtxP1A64v7JDBxpAbpTSnhlq9/DHmQ28=` |
| Reference runtime | Go 1.27.1, `CGO_ENABLED=0`, `GOWORK=off` |
| Rust toolchain | 1.98.1, edition 2021 |

Go-DomDistiller v1.0.0 tags this exact reference commit. Rust-DomDistiller 1.0.0
retains the pseudo-version in its development oracle so existing fixtures and
benchmark provenance stay reproducible. Release packaging changes the crate
version and installation documentation, not the measured extraction code or
dependency versions.

The reference module is in [tools/go-reference](tools/go-reference). Its nested
module path permits importing Go-DomDistiller's internal packages. There are no
module replacements and no runtime calls to the oracle. Checksums remain enabled.

The pinned reference graph is Shiori DOM
`v0.0.0-20230515143342-73569d674e1c`, chardet
`v0.0.0-20211120154057-b7413eaefb8f`, cascadia `v1.3.2`, zerolog `v1.33.0`,
go-colorable `v0.1.13`, go-isatty `v0.0.20`, x/net `v0.29.0`, x/text `v0.18.0`,
and x/sys `v0.25.0`. The oracle checks the complete graph and platform-specific
linked build metadata. Test-only transitive modules are locked in its go.sum.

Runtime parsing uses third-party html5gum tokenization, html5ever HTML5 tree
construction, and a local adapter into the native node arena. The adapter retains
attribute order and duplicates, and applies the pinned Go parser's foreign
attribute adjustment rules. Attributes store namespace, local key, and value
separately; lookups and filters compare the local key, while rendering restores
the namespace prefix. Literal colon-containing HTML attribute names stay literal.
Both pinned Go extraction clones (`dom.Clone` and `domutil.TreeClone`) omit element
namespaces but retain attribute namespaces. The Rust extraction arena follows
that behavior; dropping attribute namespaces is not equivalent.
URL behavior is a native translation of the reference runtime's `net/url`.
Charset scoring is translated from the pinned gogs/chardet, with generated static
recognition tables. `encoding_rs` supplies standard decoding tables; local
framing, replacement, ISO-2022-JP state handling, and pinned GB18030 tables retain
the observed Go decoder behavior. Unicode normalization uses
`unicode-normalization`. All resolved Rust dependencies are in [Cargo.lock](Cargo.lock).

## Coverage

The fixture contains 44,528 entries. One entry can assert multiple intermediate
states or option results; these counts are not an extraction-quality benchmark.

| Section | Cases | Exact Checks |
| --- | ---: | --- |
| words | 260 | All three counters and selected counter |
| classifiers | 8,250 | Word/density thresholds and changed/content flags |
| filters | 2,880 | Twelve filters, block fields, labels, offsets, order |
| dom | 41 | HTML, text, visibility and display fallback |
| attributes | 360 | Foreign/HTML names, duplicates, lookup, mutation, copying, URLs, filtering and rendering |
| converters | 98 | Elements, blocks, article filtering, HTML/text/images |
| siblings | 720 | Similar-sibling and largest-with-sibling decisions |
| tables | 2,100 | Data/layout classification and exact reason |
| markup | 335 | OpenGraph, microdata, IE reading metadata and titles |
| extraction | 540 | Complete results, repeated calls, caller DOM unchanged |
| pagination | 2,139 | Both algorithms, single-direction calls and Go path-cleaned folder URLs |
| number_groups | 240 | Number-group state and direction changes |
| page_patterns | 40 | Pattern values and matching multiple document URLs |
| urls | 486 | 54 inputs against nine bases |
| readers | 43 | Charset candidate sets, parsed HTML, title and output |
| encodings | 571 | Every nonzero charset confidence score |
| decodings | 25,245 | BOMs, normalization, all single bytes, malformed sequences |
| public | 180 | Saved page, public pagination/skip options and URL formatting |

Generated randomized sections use fixed seeds 20260914 through 20260917. Float
comparisons are exact; serde_json's `float_roundtrip` feature is required. No
HTML canonicalization or tolerance hides output differences.

The saved page is the original 328,232-byte `example/sample.html` in the pinned
Go module. Its canonical URL is
`https://www.vice.com/en_uk/article/k7qpqe/how-coronavirus-is-impacting-the-arab-world`.
Go extracts 814 words. SHA-256:
`f37c2a8ea3b48f3d154b21bf0cf030f5ccef95e607561418dfc52d77fa5e7930`.
It is exercised with both pagination algorithms, with pagination enabled and
disabled. No requests to that site or its embedded assets are made by the tests.

Fixture [testdata/go-reference.json](testdata/go-reference.json) is 40,491,091
bytes. SHA-256:
`449b416673d1ed5d16ace9fa750280e835e7f9b95c8bee37c11bd675bbeb0f44`.
The expanded snapshot retains every existing case unchanged and adds the
attribute, folder-path, and corresponding public-API cases. Default tests use
this pinned snapshot; the opt-in live test regenerates it using Go and requires
an exact byte match.

## Observed Behavior

- Extraction retries from a fresh DOM copy when the first pass has fewer than
  500 content words, retaining the same filter order and exact density thresholds.
- Duplicate attribute access uses the first occurrence, and ordering is retained.
  Access, mutation and removal compare Go's local key, even for namespaced SVG
  and MathML attributes. Updating a value retains its namespace; removing one
  occurrence exposes the next matching key. Filtering retains the complete
  attribute record rather than flattening its name.
  The converter's detached-JavaScript-anchor traversal effect is reproduced.
- Previous/next folder scoring uses Go's `path.Dir` lexical cleaning, including
  repeated slashes, dot segments and embedded URL paths. It does not apply that
  cleaning to the current URL or candidate URL themselves.
- Text generation does not assign a page URL to text elements, so text-link
  targets differ from the HTML builder exactly as in Go.
- Table row counts read `rowspan` from `tr`; columns count `td`, not `th`.
- OpenGraph requires its mandatory fields; microdata accepts the source's exact
  `http://schema.org/` types rather than adding HTTPS or JSON-LD support.
- Reader charset declarations are not trusted as an override: the pinned
  statistical detector decides the charset. NFD, soft-hyphen removal, then NFC
  follow the actual Shiori pipeline. Undefined legacy code-page bytes and
  multibyte error consumption differ from newer WHATWG decoders and are qualified
  separately.
- GB18030 retains Go's wider second-byte range and its supplementary-plane limit.
  Source expressions are retained rather than retuning or correcting the decoder.

## Nondeterminism and API Scope

Go's charset detector executes recognizers concurrently and keeps the first
highest-confidence result. Rust executes them in a fixed order. Score parity is
checked independently of winner selection. Reader snapshots record all tied
highest-scoring charsets and reject ties that decode differently; the ASCII and
zero-byte reader ties in this fixture decode identically. Genuinely ambiguous
byte streams remain a compatibility risk: exact scores alone do not prove equal
decoded output. A different deterministic result is not an accepted exception.

Go's page-number detector iterates candidate maps, so Go can select different
equal-ranked candidates across executions. Rust uses stable key ordering.
Ranking and candidate semantics must still match; nondeterminism is not
permission to change either rule.

The parser and Unicode dependencies differ from Go's and require compatibility
handling where their behavior diverges. Malformed HTML, foreign content, invalid
UTF-8 URL paths, Unicode version differences and extreme nesting are parity risks
to investigate, not excluded input classes. The mutable node arena requires valid
acyclic indices when edited directly. Extracted HTML is not a security sanitizer.

The supplied-document API is implemented. The HTTP helper, Go logging flags and
output, and granular parser timing subentries are omitted. Timing values are
Rust diagnostics, not cross-language parity fields. File-open errors preserve
Go's prefix, while the operating system's underlying I/O error wording differs.
Invalid URL strings return a Rust error because Go accepts an already parsed
URL object. Trafilatura integration and post-sanitization comparisons remain a
separate milestone. The corpus benchmark below adds measured evidence; no release
or publication is implied.

## Corpus Benchmark

The 2026-09-15 comparison reuses the complete manifest and scoring rules from
[content-extractor-benchmark](https://github.com/markusmobius/content-extractor-benchmark/tree/466fdbee8a504441eb78ed11d71c1da220681cab)
at `466fdbee8a504441eb78ed11d71c1da220681cab`. It contains 983 pages, 2,935 positive
snippets, and 2,948 negative snippets. The manifest's comments and metadata are
not scored. Duplicate snippets count independently. Matching is case-sensitive
substring containment in extracted text. Empty output misses every positive
snippet and excludes every negative snippet.

Global counts are summed before computing precision = TP / (TP + FP), recall =
TP / (TP + FN), and F1 = 2 TP / (2 TP + FP + FN). Both engines produce TP 2,535,
FN 400, FP 375, TN 2,573 in all three pagination settings. Precision is
0.8711340206185567, recall 0.8637137989778535, F1 0.8674080410607357, and accuracy
0.8682644909060003. The rounded values reproduce the upstream README's results.
These labeled-content scores are separate from exact cross-language agreement.

The Go exporter parses the pinned manifest with Go's AST API. It does not import
the benchmark repository's dependency graph or its older DomDistiller version.
The reference remains the version and runtime graph in the authority table above.
Raw pages are decoded once using the pinned Go reader pipeline, including
NFD/soft-hyphen removal/NFC, and the identical normalized HTML is supplied to both
native parsers. All top-confidence charset candidates must yield identical
normalized bytes. The export records sorted charset candidates and SHA-256 for
each original file and normalized input. No page or extraction error is skipped.

The normalized corpus was exported twice with identical bytes. SHA-256:
`0e8b21bc8c28a88a90d891d91020a21aab95a2cfa83f761dd2b1642d98c757ea`.
The corpus and full extracted articles stay in ignored `target/benchmark/` files;
they are not bundled as newly licensed library content.

### Exact Agreement

The benchmark compares six fields without canonicalization: text, title, word
count, serialized result-node HTML, next-page URL, and previous-page URL. All six
fields agree on all 983 pages for all three settings, with zero output differences.
This benchmark does not compare metadata or image lists; those fields have
separate coverage in the offline differential fixture.

The former SVG HTML difference was fixed by preserving Go attribute namespace
semantics throughout parsing, lookup, filtering and serialization. The former
archived-page previous-link difference was fixed by using Go's path-cleaned
folder in pagination scoring. Neither fix is specific to a site or fixture.
The runner now saves diagnostics and fails on any differing output field before
timing that setting; equal quality scores cannot conceal a parity regression.

### Timing Method

The [README](README.md#quality-and-speed) contains the quality and speed tables
comparing Go with the corrected Rust implementation, both rebuilt for this run.
Raw nanosecond samples, order, warmups, counts, differences, ranges, source hashes,
binary hashes, and machine/toolchain details are retained in
[testdata/benchmark-parity-results.json](testdata/benchmark-parity-results.json).
Its SHA-256 is
`77820bf2dfb94e8f8f783d80cfc4729c2dc9065b1d49b97d20f4174c06c3cce3`.
The earlier measurement records remain unchanged and are not the current table's
source. The retained hashes identify the pre-release manifest and lockfile;
the 1.0.0 release changes only the root crate version in those files, with no
runtime source or dependency changes after measurement.

The machine is an AMD Ryzen AI 7 PRO 350 with 16 logical CPUs, running x86_64 Linux
6.18.33.2-microsoft-standard-WSL2. Both workers inherit affinity to logical CPU 2;
Go uses `GOMAXPROCS=1` and `CGO_ENABLED=0`. Rust uses its standard Cargo release
profile, Go its normal optimized build, without native-CPU tuning. Versions are
Go 1.27.1 and Rust 1.98.1 (LLVM 22.1.8). The Python 3.14.4 stdlib orchestrator only
coordinates native workers and writes results; it does not perform extraction.

Each worker loads and parses all normalized pages before announcing readiness.
Per mode, an untimed-for-reporting output pass qualifies results, then two warmup
and ten measured full-corpus passes alternate Go/Rust and Rust/Go order. The
workers persist between passes; allocation, normal Go garbage collection, and Rust
destruction are part of their native lifecycle. Counts are checked on every pass.
The reported ratio divides the two medians; it is not a statistical guarantee.

Timers surround extraction from the pre-parsed DOM and snippet evaluation, matching
the upstream benchmark's workload. Options are prepared before timing. Input I/O,
charset decoding, initial DOM parsing, process startup, IPC, and extra caller-side
result HTML serialization are excluded. Output construction/serialization/reparsing
performed internally by `Apply`/`apply` remains included. Quality-pass elapsed
values include additional output collection and are never used as speed samples.

Observed min-max times per 983-page pass were 3,828-5,110 ms Go and 1,319-2,053 ms
Rust with pagination skipped; 4,844-7,739 ms Go and 1,758-3,614 ms Rust for
previous/next; and 4,031-4,558 ms Go and 1,387-2,390 ms Rust for numbered pages.
This is one-machine WSL2 evidence, not an end-to-end ingestion or memory benchmark.

### Benchmark Reproduction

Use a Linux source checkout (including WSL2), Git, Python 3.11+, `lscpu`, Go
1.27.1, and the pinned Rust toolchain installed through rustup. The small Python
orchestrator is source-checkout tooling, intentionally outside the crate package's
explicit include list. The native workers and saved measurements are packaged.
No external Python packages are needed.

From a fresh crate checkout:

```sh
git -c core.autocrlf=false clone https://github.com/markusmobius/content-extractor-benchmark.git target/benchmark/upstream
git -C target/benchmark/upstream checkout --detach 466fdbee8a504441eb78ed11d71c1da220681cab
GOWORK=off CGO_ENABLED=0 GOTOOLCHAIN=go1.27.1 go -C tools/go-reference run -mod=readonly . -benchmark-source ../../target/benchmark/upstream -output ../../target/benchmark/corpus.json
GOWORK=off CGO_ENABLED=0 GOTOOLCHAIN=go1.27.1 go -C tools/go-reference build -mod=readonly -o ../../target/benchmark/go-worker .
cargo build --release --locked --example benchmark
python3 tools/benchmark.py --cpu 2 --warmups 2 --samples 10 --output target/benchmark/results.json
```

Choose a logical CPU allowed by the local affinity mask. Fetch dependencies first
when working offline. Omit timing with `--warmups 0 --samples 0`, select a setting
with `--mode skip`, `--mode prev-next`, or `--mode page-number`, or use `--limit 8`
for a smoke check only. A first-N smoke subset is not a representative quality or
performance sample. The exporter refuses a different source revision or tracked
source modifications; ordinary reruns reuse the prepared corpus without fetching.

### Native Test Reuse

The original Go suite was run without source-cache edits using Go 1.27.1:

```sh
GOWORK=off CGO_ENABLED=0 go -C "$(go env GOMODCACHE)/github.com/markusmobius/go-domdistiller@v0.0.0-20240926050704-25b8d046ffb4" test -mod=readonly -count=1 ./...
```

All packages passed; the source contains 47 test files and 383 test functions.
The Rust `original_go_document_title_tests` test directly translates 22 of the
23 original title scenarios, retaining their names and expected results. The
nil-root-only Go scenario has no equivalent in the non-null Rust document API.
These run offline along with the 44,528 Go-generated differential entries. This
does not claim that all 383 Go test functions were literally translated to Rust.
Both native benchmark scorers separately test duplicate labels and empty output.

## Reproduction

Run from the crate root with Go 1.27.1 installed:

```sh
GOWORK=off CGO_ENABLED=0 go -C tools/go-reference run -mod=readonly . -output ../../target/go-reference-live.json
GO_DOMDISTILLER_REFERENCE="$PWD/target/go-reference-live.json" cargo test --locked --lib live_go_parity -- --ignored --nocapture
cargo test --locked --offline
```

On PowerShell, set process-local `GOWORK=off` and `CGO_ENABLED=0` and use an
absolute `GO_DOMDISTILLER_REFERENCE` path. `GO` can name the executable for the
live test. The tools resolve Go through `runtime.GOROOT()` for module queries.

To reproduce generated recognition and GB18030 tables:

```sh
go -C tools/go-reference run -mod=readonly . -encoding-tables ../../target/encoding-tables.rs
rustfmt --edition 2021 target/encoding-tables.rs
```

Compare that file with [src/encoding/tables.rs](src/encoding/tables.rs). The
exporter uses Go's AST, not regular-expression source rewriting. Notices may be
reproduced into an isolated directory with `-notices ../../target/notices`; that
optional command fetches the complete Chromium notice at its immutable revision.

Formatted generated-table SHA-256:
`6d445b77680af310e7f4cc9486d1a53176f619692238d1e87b63bf02becabad2`.
Independent export followed by the pinned rustfmt reproduced these bytes exactly.

## Attribution

[testdata/provenance.json](testdata/provenance.json) records exact source
locations, sizes, and SHA-256 fingerprints. Retained notices are byte-preserved.
The pinned Go Chromium license file is empty; its original is retained and the
complete Chromium BSD/Apache terms are added from revision
`2a180397710719913340a12804affc65b789275e`. Boilerpipe's own copyright statement and
NOTICE remain present. Shiori, chardet, ICU, and Go adaptation notices are retained
separately. See [NOTICE](NOTICE) for the modification and source mapping.

The benchmark scorer and manifest interpretation are adapted from the pinned
content-extractor-benchmark under Apache-2.0, with its Go-Trafilatura benchmark
lineage retained in [NOTICE](NOTICE). Its saved third-party pages retain their
original rights; the benchmark source license does not relicense their content.

## Local Verification

Local verification of the parity corrections on 2026-09-15, with Rust 1.98.1 and
Go 1.27.1:

| Gate | Observed Result |
| --- | --- |
| Linux x86_64 / WSL debug, locked/offline | 27 library tests and one benchmark-scorer test passed |
| Linux x86_64 / WSL release, locked/offline | 27 library tests and one README doctest passed |
| Native Windows x86_64 / GNU release | 27 library tests and one benchmark-scorer test passed |
| Linux formatting / Clippy / release build | Pinned rustfmt, all-target `-D warnings`, and build passed |
| Go oracle | Tests, build, vet, module verification, and `go mod tidy -diff` passed |
| Fresh live Go fixture | Exact byte match to all 44,528 retained cases on Linux |
| Source notice and page hashes | Retained notices and all public HTML inputs verified |
| Full corpus comparison | All six output fields equal on 983/983 pages in all three modes |
| Comparison failure gate | Deliberate changes to each output field rejected after saving diagnostics |
| `cargo package --locked --offline --allow-dirty` | 59 files built and verified, including fixture, measurements and notices |
| Extracted-package release tests | 27 offline tests and one README doctest passed in an isolated build directory |

The package contains the complete offline fixture and reference tooling;
compressed size is approximately 1.2 MiB. Dependencies were cached for offline
checks. The opt-in live check is ignored in ordinary test runs and was executed
separately on Linux. Earlier initial-port checks also exercised an independent
packaged consumer, fresh Windows Go export and generated encoding tables; those
are not presented as fresh checks of this correction. The CI configuration
covers Linux and Windows/MSVC; hosted CI, native MSVC, macOS, ARM64, and older
Rust versions have not been executed for these changes. The 1.0.0 release ships
the supplied-document API, complete offline fixture, notices and retained
Go comparison described above.