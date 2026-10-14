# Fnine

A self-hosted EPUB reading service written in Rust. Upload, manage, and read your ebooks right in the browser — no database required.

## Features

- **EPUB Upload** — Supports EPUB 2 and EPUB 3 formats. Metadata (title, author, cover) is extracted automatically.
- **Online Reader** — Clean chapter-by-chapter reading experience with a built-in table of contents sidebar.
- **Reading Progress** — Scroll position is saved automatically so you can resume where you left off.
- **Annotations & Notes** — Highlight text, choose a color, and attach personal notes to any passage.
- **Search** — Find books by title or author instantly.
- **Multi-language** — English and Chinese (中文) UI. Easy to add more.
- **Themes** — Light and dark reading themes.
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
  -p 3000:3000 \
  -v fnine-data:/app/data \
  ghcr.io/deadpoetspoon/fnine:latest
```

### From Source

**Prerequisites:** Rust 1.96+ (edition 2024).

```bash
git clone https://github.com/DeadPoetSpoon/fnine.git
cd fnine
cargo run --release
```

The server will start at `http://0.0.0.0:3000`.

## Configuration

Fnine is configured via environment variables:

| Variable        | Default    | Description                  |
| --------------- | ---------- | ---------------------------- |
| `FNINE_HOST`    | `0.0.0.0`  | IP address to bind to        |
| `FNINE_PORT`    | `3000`     | Port to listen on            |
| `FNINE_DATA_DIR`| `./data`   | Directory for persistent data |

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
│   │   ├── search.rs     # Book search
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
│   ├── search.html          # Search results
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
│   ├── annotations/         # Per-book annotation TOML files
│   ├── books.toml           # Book metadata index
│   ├── progress.toml        # Reading progress per book
│   ├── settings.toml        # User settings
│   └── annotations.toml     # (reserved)
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
| `GET`  | `/search?q=`                          | Search books               |
| `GET`  | `/settings`                           | Settings page              |
| `POST` | `/settings`                           | Save settings              |
| `POST` | `/settings/fonts`                     | Upload a font file         |
| `POST` | `/settings/fonts/delete`              | Delete a font file         |
| `POST` | `/api/progress`                       | Save reading progress      |
| `GET`  | `/api/recent`                          | Last-read book (JSON)       |
| `GET`  | `/api/book/{id}/annotations`          | List annotations           |
| `POST` | `/api/book/{id}/annotations`          | Create annotation          |
| `POST` | `/api/book/{id}/annotations/{aid}`    | Delete annotation          |

## License

MIT
