# Rust-DomDistiller Maintenance

Read these instructions before changing README.md, UPSTREAM.md, CHANGELOG.md,
benchmark claims or releases. Inspect current files and git status; preserve
unrelated edits and follow explicit user constraints.

## Library Identity

The behavioral reference is the pinned Go-DomDistiller main-branch implementation,
not Python or the original Chromium Java code. Explain source ancestry separately
from the current compatibility target. The library extracts from supplied HTML,
including pagination, without browser layout, fetching or internal worker threads.
Its DOM types can be used without running its extraction algorithm. Removing
DomDistiller as a Trafilatura fallback does not remove standalone extraction.
Do not claim DomDistiller performs no cleanup; candidate reuse changes its input.

## Document Roles

- README.md: purpose, scope, usable examples, key differences and concise current
  quality/speed. Avoid application-worker incidents, oracle hashes and audit logs.
- UPSTREAM.md: source/dependency pins, deliberate deviations, controlled evidence,
  reproduction commands, hashes, coverage and unresolved compatibility issues.
- CHANGELOG.md: dated/versioned user-visible changes and reasons. Explicitly
  label documentation-only releases; keep historical entries historical.
- Release notes: the same release-facing changes, benchmark definitions and
  limitations as the README/changelog, with links to detailed evidence.
- AGENTS.md: durable instructions, not current results or work-in-progress.

## Six-Repository Benchmark Contract

Follow the complete
[benchmark maintenance guide](https://github.com/markusmobius/content-extractor-benchmark/blob/master/AGENTS.md).
Coordinate go-domdistiller, rust-domdistiller, go-readabilityV2, rust-readability,
go-trafilatura and rust-trafilatura, not only this library's language pair.

1. Every README has `## Current Quality and Speed` with the same six-engine
	comparison from one completed shared-suite report. Use measured version labels.
2. Columns: `Extractor`, `Go Version`, `Rust Version`, `Go ms/page`,
	`Rust ms/page`, `Go/Rust`. Rows: Readability, DomDistiller, Trafilatura FAST.
	Display milliseconds/page to three decimals and Go/Rust ratios to two decimals.
3. Read structured JSON and compute ratios from unrounded means. The current
	shared protocol uses all four measured passes after one warmup. Never mix
	dates, environments, modes, means/medians or selected/all-pass aggregates.
4. Report parsing separately, charged once per language/page. Include decoding,
	normalization, DOM construction and any separately required Trafilatura tree.
	State corpus/counts, options, hardware, toolchains and included/excluded work.
	DomDistiller pagination must be identified as on/off and by algorithm.
5. Keep non-FAST Trafilatura as a separate comparison with its own report. A
	paired language ratio is not an isolated version speedup or request latency.
6. Report named-corpus F1 percentages to five decimals, retaining errors in the
	denominators. Do not average different scoring definitions. Matching text
	scores do not prove byte-identical HTML or metadata; preserve known differences.
7. Link immutable reports/commits and preserve old artifacts unchanged. Clearly
	date old standalone timing/allocation tables; do not compare them directly
	to the current shared-input extraction boundary.
8. Documentation-only patches retain actual measured versions; do not relabel
	old measurements or rerun benchmarks just to change wording. Trafilatura's
	fallback selection frequency is not DomDistiller's standalone accuracy.

## Editing and Crate Publication

1. Identify scope/evidence before editing. Documentation does not authorize
	runtime or dependency changes. Coordinate the three document roles and all
	six common benchmark sections; use plain library-facing prose.
2. Validate numbers, version labels, API names and links. Compare common sections
	and run `git diff --check`. Use the pinned toolchain and relevant existing
	format/doc-test/test/lint gates; report optional/unrun coverage accurately.
3. Obtain authorization before commits, pushes, versions, tags or publication.
	Never move a published tag or replace an immutable crate archive.
4. Finalize README, UPSTREAM, CHANGELOG and AGENTS before packaging. A
	crates.io README update requires a new version; changing GitHub release text
	cannot update its archive. Inspect Cargo's actual package file list.
5. For documentation-only patches, change package identity/its lockfile entry
	and current installation links without changing runtime source or dependency
	pins. Keep benchmark rows labeled with versions that were actually timed.
6. Inspect the clean-commit crate archive, excluding temporary tools/secrets.
	Compare runtime sources with the preceding published archive and intended
	docs with the new commit. Confirm package ownership/version availability.
7. Verify the published checksum, source/doc bytes, normal registry installation
	and GitHub release page. A source tag is not crates.io publication.
8. Hash exact Git/published bytes, not assumed Windows checkout bytes. Preserve
	historical artifacts and unrelated edits. Never print credentials or claim
	hosted CI success without observing it.
9. Report actual validation, publication state and remaining limitations. Keep
	detailed source receipts in UPSTREAM/reports rather than the README/changelog.