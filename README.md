# Fnine

A self-hosted EPUB reading service written in Rust. Upload, manage, and read your ebooks right in the browser — no database required.

## Features

- **EPUB Upload** — Supports EPUB 2 and EPUB 3 formats. Metadata (title, author, cover) is extracted automatically.
- **Online Reader** — Clean chapter-by-chapter reading experience with a built-in table of contents sidebar.
- **Publisher Endnotes** — Select a footnote/endnote reference to open its actual text in an accessible popup, including separate note documents, CSS-styled superscripts, and locally grouped circled-number notes without links.
- **Reading Progress** — Scroll position is saved automatically so you can resume where you left off.
- **Annotations & Notes** — Highlight text, choose a color, and attach personal notes to any passage.
- **Library Controls** — Continue reading, sort by title/author/upload/last-read, and filter unread, reading, or finished books.
- **Annotation Editing & Export** — Edit notes/colors and download your annotations as Markdown.
- **Multi-language** — English and Chinese (中文) UI. Easy to add more.
- **Themes** — Light, dark, and sepia reading themes; saved language/theme settings apply by default.
- **Custom Fonts** — Upload your own `.ttf` or `.woff2` font files for the reader.
- **No Database** — All data is persisted as plain TOML files on disk. Zero configuration, easy to back up.
- **In-memory Caching** — Chapter content and book lists are cached in memory for fast responses.
- **Docker Ready** — Multi-stage Dockerfile with `cargo-chef` for efficient builds. Compressed final image based on Alpine.

## Screenshots

*Screenshots coming soon.*

## Quick Start

### With Docker

```bash
docker run -d \
  --name fnine \
  -p 127.0.0.1:3000:3000 \
  -v fnine-data:/app/data \
  ghcr.io/deadpoetspoon/fnine:latest
```

### From Source

**Prerequisites:** Rust 1.96+ (edition 2024).

```bash
git clone https://github.com/DeadPoetSpoon/fnine.git
cd fnine
cargo run --locked --release
```

The server will start at `http://127.0.0.1:3000`. Run from the repository root so the `static/` assets are available.

## Configuration

Fnine is configured via environment variables:

| Variable        | Default    | Description                  |
| --------------- | ---------- | ---------------------------- |
| `FNINE_HOST`    | `127.0.0.1`  | IP address to bind to (container default: `0.0.0.0`) |
| `FNINE_PORT`    | `3000`     | Port to listen on            |
| `FNINE_DATA_DIR`| `./data`   | Directory for persistent data |

## Reading and Storage Guarantees

Fnine is a **text-first** reader: paragraphs, entities, Unicode, navigation labels, and note text are supported. Publisher CSS, illustrations, equations, and rich note formatting are not reproduced. Broken note targets show an explicit unavailable message; Fnine never retrieves external note URLs or renders uploaded HTML directly.

Only **one process** may use a data directory. Requests coordinate TOML mutations with cache access; saves use unique, synced temporary files and atomic replacement. Import/deletion journals are recovered at startup. This is not a distributed database or a promise of multi-file transactional snapshots.

Older books migrate on first reading. Original metadata/progress/annotations are backed up under `data/backups/`; uniquely located annotations are reattached, ambiguous annotations remain available with a review warning. A progress location that cannot be mapped safely resumes at the beginning of that chapter. Existing font filenames are migrated into an inventory with generated IDs.

Limits: 50 MiB EPUB multipart requests, 10 MiB font files (plus bounded multipart overhead), 256 KiB JSON/forms, 16 KiB per annotation quote/note, 16 MiB per expanded EPUB resource, 256 MiB total expansion, and 10,000 archive entries. Chapter caching is bounded to 64 MiB, navigation caching to 4 MiB, and parsing to two blocking jobs. Notes are text-only and bounded to 64 KiB each / 4 MiB per chapter.

## Private Deployment and Backups

There is **no authentication**: anyone able to reach the server can read, upload, edit, or delete data. Keep the default loopback publication or use a trusted private network/authenticated reverse proxy. Deliberate LAN access requires `FNINE_HOST=0.0.0.0` for source runs and an intentional host port binding for Docker.

The container runs as **root by default** for compatibility with existing data mounts; upgrading does not require changing ownership to UID `10001`. For optional non-root operation, add `--user fnine:fnine` to `docker run` before the image name. This account uses UID/GID `10001`; stop Fnine and ensure the entire mounted data directory is writable by that identity before switching. Non-root operation limits container privileges but is not the default.

Mutation requests with a browser Origin must match the host and scheme; cross-site Fetch Metadata requests are rejected. Non-browser clients without Origin remain supported. A trusted HTTPS reverse proxy must preserve Host and set `X-Forwarded-Proto: https`; never expose this unauthenticated service publicly merely because it has Origin checks.

To back up: stop Fnine, copy the **entire** data directory (including EPUBs, fonts, TOML, journals, and migration backups), then restart. To restore: stop Fnine, restore that complete copy, correct ownership, and restart one instance. Do not edit TOML behind a running instance or copy a live directory as if it were a consistent snapshot. The data directory must be trusted local storage, not writable by untrusted users.

## Verification

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release
npm ci
npx playwright install chromium
npm run test:browser
```

Browser tests use an installed `/usr/bin/chromium` when available; set `FNINE_BROWSER` to override it. Font-loading checks use an installed DejaVu, Liberation, or iA Writer TTF; set `FNINE_TEST_FONT` to a valid TTF path otherwise. Node/Playwright are development-only and are not required by the server. Synthetic EPUB fixtures cover EPUB 2/3 endnotes, reciprocal numeric links, CSS superscripts, repeated local circled markers, and misprinted labels with explicit list values. Ambiguous or missing note bodies are reported as unavailable rather than guessed.

To audit local EPUBs containing notes without changing library data, and optionally test their popups in an isolated Chromium test library:

```bash
FNINE_TEST_BOOKS=data/books cargo test --locked --offline local_epub_endnote_regression -- --ignored --nocapture
FNINE_TEST_BOOKS=data/books npm run test:browser
```

The audit checks every spine chapter for unresolved references and unchanged chapter text/coordinates. It expects each EPUB in the directory to contain at least one recognizable note. The browser check opens a real note popup from each EPUB; original files and library records remain untouched.

## Project Structure

```
fnine/
├── src/
│   ├── main.rs           # Entry point, router setup
│   ├── config.rs         # Environment configuration
│   ├── state.rs          # Shared application state
│   ├── error.rs          # Unified error type
│   ├── cache/
│   │   └── mod.rs        # In-memory cache
│   ├── db/
│   │   ├── mod.rs
│   │   ├── store.rs      # Generic TOML-backed persistent store
│   │   ├── books.rs      # Book data model
│   │   ├── progress.rs   # Reading progress data model
│   │   ├── annotations.rs# Annotation data model
│   │   └── settings.rs   # User settings data model
│   ├── epub/
│   │   ├── mod.rs
│   │   └── parser.rs     # EPUB metadata & chapter extraction
│   ├── handlers/
│   │   ├── mod.rs
│   │   ├── library.rs    # Home, upload form, book detail, cover image
│   │   ├── reader.rs     # Reader page with chapter navigation
│   │   ├── api_books.rs  # Upload / delete book API
│   │   ├── api_progress.rs  # Save reading progress API
│   │   ├── api_annotations.rs# CRUD annotations API
│   │   └── api_settings.rs  # Settings page & font upload
│   └── i18n/
│       ├── mod.rs
│       ├── translations.rs  # Translation loading & flattening
│       ├── en.toml          # English translations
│       └── zh.toml          # Chinese translations
├── templates/
│   ├── base.html            # Base layout with nav
│   ├── index.html           # Library home (book grid)
│   ├── upload.html          # Upload form
│   ├── book_detail.html     # Book detail with annotations
│   ├── reader.html          # Online reader
│   ├── settings.html        # Settings page
│   └── components/
│       └── book_card.html   # Reusable book card component
├── static/
│   ├── css/                 # Stylesheets
│   └── js/                  # Client-side JavaScript
├── data/                    # Default data directory (mounted as volume in Docker)
│   ├── books/               # Stored EPUB files
│   ├── covers/              # Extracted cover images
│   ├── fonts/               # User-uploaded fonts
│   ├── fonts.toml           # Font inventory (generated IDs and display names)
│   ├── staging/             # Bounded upload staging
│   ├── imports/             # Recoverable import journals
│   ├── deletions/           # Recoverable deletion journals
│   ├── backups/             # Legacy normalization backups
│   ├── annotations/         # Per-book annotation TOML files
│   ├── books.toml           # Book metadata index
│   ├── progress.toml        # Reading progress per book
│   └── settings.toml        # User settings
├── Dockerfile               # Multi-stage Docker build
├── Cargo.toml               # Rust dependencies
└── .github/workflows/       # CI/CD pipelines
    ├── ci.yml               # Format, lint, build, test
    └── docker.yml            # Build & push Docker image
```

## Technology Stack

| Component     | Crate / Technology            |
| ------------- | ----------------------------- |
| Web Framework | [axum](https://crates.io/crates/axum) 0.8 |
| Templating    | [askama](https://crates.io/crates/askama) 0.16 |
| EPUB Parsing  | [rbook](https://crates.io/crates/rbook) 0.7 |
| Async Runtime | [tokio](https://crates.io/crates/tokio) 1.52 |
| Serialization | [serde](https://crates.io/crates/serde) + [toml](https://crates.io/crates/toml) |
| Middleware    | [tower-http](https://crates.io/crates/tower-http) 0.7 |
| Logging       | [tracing](https://crates.io/crates/tracing) 0.1 |
| IDs           | [uuid](https://crates.io/crates/uuid) 1.23 (v4) |
| Timestamps    | [chrono](https://crates.io/crates/chrono) 0.4 |

## API Overview

| Method | Path                                  | Description                |
| ------ | ------------------------------------- | -------------------------- |
| `GET`  | `/`                                   | Library home page          |
| `GET`  | `/upload`                             | Upload form                |
| `POST` | `/upload`                             | Upload an EPUB file        |
| `GET`  | `/book/{id}`                          | Book detail page           |
| `GET`  | `/book/{id}/download`                 | Download the original EPUB |
| `POST` | `/book/{id}/delete`                   | Delete a book              |
| `GET`  | `/book/{id}/read`                     | Redirect to last chapter   |
| `GET`  | `/book/{id}/read/{chapter}`           | Read a specific chapter    |
| `GET`  | `/covers/{id}`                        | Serve cover image          |
| `GET`  | `/settings`                           | Settings page              |
| `POST` | `/settings`                           | Save settings              |
| `POST` | `/settings/fonts`                     | Upload a font file         |
| `POST` | `/settings/fonts/delete`              | Delete a font file         |
| `POST` | `/api/progress`                       | Save reading progress      |
| `GET`  | `/api/recent`                          | Last-read book (JSON)       |
| `GET`  | `/api/book/{id}/annotations`          | List annotations           |
| `POST` | `/api/book/{id}/annotations`          | Create annotation          |
| `POST` | `/api/book/{id}/annotations/{aid}`    | Delete annotation          |
| `PATCH` | `/api/book/{id}/annotations/{aid}`   | Edit note and highlight color |
| `GET` | `/book/{id}/annotations/export`       | Download Markdown annotations |

## License

MIT
