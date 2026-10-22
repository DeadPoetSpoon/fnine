# Fnine — consolidated implementation plan

Updated: October 22, 2026. Release: v0.7.0. Target: private, single-user, text-first EPUB reading.
Keep Rust/Axum/Askama, TOML persistence, and compatible existing routes. No accounts,
database rewrite, raw EPUB HTML rendering, or network retrieval of publisher content.

Status legend: implemented, verified by the checks recorded below, unavailable, skipped.
All implementation work in sections A–C is complete. Verification limitations are
recorded separately rather than treating an unexecuted check as a pass.

## A. Data safety and runtime foundations

- Implemented: serialize whole read/modify/write operations and cache publication; reject
  second-process access to a data directory; unique synced atomic replacement files.
- Implemented: validate IDs, book/chapter existence, finite normalized positions, ranges,
  annotation text (16 KiB), colors, languages, themes, fonts, and 12–32 px font sizes.
- Implemented: generated font identifiers/inventory, filesystem confinement and symlink rejection.
- Implemented: bounded streaming uploads (EPUB 50 MiB, font 10 MiB), 256 KiB JSON/forms;
  archive limits (16 MiB/resource, 256 MiB total, 10,000 entries); two parsing workers.
- Implemented: 64 MiB chapter cache, navigation cache, blocking parsing off async workers.
- Implemented: staged imports with cleanup; recoverable deletion of every associated record,
  file, and cache; recent-book selection ignores orphaned progress.
- Implemented: safe image-header inspection, clear startup failures, graceful shutdown,
  request tracing, and non-sensitive HTTP errors.

## B. Correct extraction and actual endnotes

- Implemented: parser-backed text extraction preserves paragraphs, entities, Unicode,
  and bracketed numbers; strips script/style content without rendering raw HTML.
- Implemented: structured chapter text/reference runs and resolved note records; EPUB
  noteref/doc-noteref and conventional superscript references to note containers.
- Implemented: resolve same/cross-document targets, including non-spine manifest resources,
  relative paths, encoded fragments, IDs and named anchors. Never fetch remote content,
  escape the archive, follow backlink cycles, or display an ID as a note body.
- Implemented: extract complete multi-paragraph notes without backlink controls; cache them
  with chapters. Missing targets have an explicit unavailable state; no new note API.
- Implemented: publisher CSS superscripts and reciprocal numeric links with arbitrary IDs;
  remove numeric backlinks and avoid treating note-body backlinks as narrative references.
- Implemented: match unlinked circled markers within local list groups, including repeated
  numbering, misprinted labels corrected by explicit list values, and a single note split
  across adjacent numbered items. Ambiguous matches remain unavailable; chapter text is retained.
- Implemented: accessible mobile-friendly popup with actual note text, keyboard activation,
  Escape/outside-click/close dismissal and focus return. Popup text lives outside chapter
  coordinates; opening a note never navigates or changes reading progress.
- Implemented: version normalization; back up legacy records, map progress using old/new
  text alignment, remap only unambiguous annotation quotes, retain unresolved records.

## C. Reader, library, and deployment

- Implemented: real EPUB navigation labels and spine mapping; coherent accessible mobile TOC.
- Implemented: query override → saved settings → defaults; safe generated custom-font CSS.
- Implemented: browser-compatible character coordinates; queued saves on chapter entry,
  scroll, visibility/page exit; layout/font-aware restore and visible save failures.
- Implemented: annotation color controls, editing, overlapping highlights and Markdown export.
  Add PATCH /api/book/{id}/annotations/{aid} and GET /book/{id}/annotations/export.
- Implemented: continue-reading, title/author/upload/last-read sorting and
  unread/reading/finished filters (finished: final chapter >=99%).
- Implemented: English/Chinese UI additions, document language and accessible labels.
- Implemented: source loopback default, deliberate container/LAN exposure, mutation Origin
  validation, root-default container with optional `--user fnine:fnine` operation,
  tagged-revision verification before publication.
- Implemented: both READMEs cover text-first limitations, configuration, minimum
  Rust, locked builds, single-process storage and stop/copy/restart backup/restore.

## Verification gates

- Verified: concurrent annotation updates, cache invalidation/eviction, serialization
  failure preserving the original store, corrupt-store rejection, exclusive directory lock.
- Verified: validation, symlink confinement, malformed/oversized archives, streaming
  upload cleanup, generated font IDs, import/delete journal recovery and associated cleanup.
- Verified: extraction and note fixtures for EPUB 2/3, relative/encoded/named targets,
  repeated/duplicate labels, non-spine notes, long notes, missing/cyclic/external links.
- Verified: marker-only target IDs expand to the complete surrounding note paragraph.
- Verified: legacy compatibility, backups, progress mapping and unresolved annotations.
- Verified in Chromium: actual and long scrollable note text, keyboard/mobile interaction, progress,
  fonts, overlapping annotations, preferences, export and error feedback.
- Verified: formatting, strict Clippy, Rust tests and release build (commands/results below).
- Unavailable: local container build/start smoke test; Docker socket access is denied.
  CI and tagged-image workflows include verification before image publication.
- Skipped: Firefox testing, explicitly requested by the user. No Firefox result is claimed.

## Verification record

- Release v0.7.0 revalidation: formatting, strict Clippy, 24 release-mode Rust tests,
  release build, real-book audit (773 resolved references), workflow syntax checks, and
  Chromium with both supplied EPUBs passed. Browser tests await the dialog close event
  before refocusing another note, avoiding a deferred focus-return race in the test harness.
- `cargo fmt --all -- --check`
- `cargo clippy --locked --offline --all-targets -- -D warnings`
- `cargo test --locked --offline`: 24 passed; two opt-in helpers ignored (fixture generator
  and local EPUB audit), both separately executed successfully.
- `cargo build --locked --offline --release`
- `npm run test:browser`: Chromium suite passed, including real TTF upload/loading,
  multi-paragraph and scrollable mobile notes, missing-note feedback, focus return,
  unchanged chapter coordinates, annotation creation/edit/overlap/export, saved preferences,
  mobile TOC, progress restoration, library filtering and visible save errors.
  The suite explicitly invokes the ignored fixture generator successfully.
- `FNINE_TEST_BOOKS=data/books cargo test --locked --offline local_epub_endnote_regression -- --ignored --nocapture`:
  all 27 chapters of 百年孤独【中文版】 and 83 chapters of 世界名著名译文库：托尔斯泰集
  audited; respectively 76 and 697 references resolve to note bodies, with zero unavailable
  notes and identical canonical chapter text. No coordinate migration is needed for this fix.
- `FNINE_TEST_BOOKS=data/books npm run test:browser`: Chromium suite passed, including new
  CSS-style and repeated unlinked-note fixtures, plus real note popup smoke tests for both
  supplied EPUBs. Original library data is read only; uploads use an isolated temporary library.
- Root-default container change: workflow YAML parsing, smoke-test shell syntax, runtime-user
  configuration checks, and `git diff --check` passed. Local Docker runtime verification is
  unavailable because access to the Docker daemon is denied, including outside the sandbox.

## Operational notes

- The request coordinator intentionally serializes handlers, including uploads, for
  private single-user correctness. This is not a multi-user throughput design.
- Ambiguous legacy annotations are retained with review flags. If progress cannot be
  aligned confidently, restoration falls back to the chapter start; backups are retained.
- Automated fixtures and the two supplied EPUB regressions do not replace real
  power-loss/filesystem fault injection or coverage of every publisher's note formatting.
- Restart the rebuilt service to clear previously cached chapters and load the parser fix;
  existing books do not need to be uploaded again. Firefox remains skipped as requested.
- Containers default to root for existing mount compatibility, as requested. The `fnine`
  UID/GID `10001` account remains an opt-in; switching to it requires writable mounted data.
  The Docker release workflow checks both identities and smoke-tests both runtime modes.

Only mark verified after executing the relevant check. Explicitly record unavailable
environment-dependent checks. Keep private EPUBs out of version control; use the opt-in
local corpus audit for additional publisher formats alongside the synthetic baseline.
