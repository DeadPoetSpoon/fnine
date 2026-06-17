use crate::config::Config;
use crate::db::books::BooksData;
use crate::db::store::Store;
use std::path::PathBuf;
use std::sync::Arc;

/// Shared application state passed to all handlers via `axum::State`.
#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub books: Arc<Store<BooksData>>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let books_path = config.data_dir.join("books.toml");
        Self {
            config,
            books: Arc::new(Store::new(books_path)),
        }
    }

    /// Path to the uploaded EPUB files directory.
    pub fn books_dir(&self) -> PathBuf {
        self.config.data_dir.join("books")
    }

    /// Path to the extracted covers directory.
    pub fn covers_dir(&self) -> PathBuf {
        self.config.data_dir.join("covers")
    }
}
