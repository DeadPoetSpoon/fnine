use crate::cache::Cache;
use crate::config::Config;
use crate::db::annotations::AnnotationsData;
use crate::db::books::{Book, BooksData};
use crate::db::progress::ProgressData;
use crate::db::settings::Settings;
use crate::db::store::Store;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub books: Arc<Store<BooksData>>,
    pub progress: Arc<Store<ProgressData>>,
    pub settings: Arc<Store<Settings>>,
    pub chapter_cache: Cache<String, String>,
    pub books_cache: Cache<(), Vec<Book>>,
    pub settings_cache: Cache<(), Settings>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let data = &config.data_dir;
        Self {
            books: Arc::new(Store::new(data.join("books.toml"))),
            progress: Arc::new(Store::new(data.join("progress.toml"))),
            settings: Arc::new(Store::new(data.join("settings.toml"))),
            chapter_cache: Cache::new(),
            books_cache: Cache::new(),
            settings_cache: Cache::new(),
            config,
        }
    }

    pub fn books_dir(&self) -> PathBuf {
        self.config.data_dir.join("books")
    }

    pub fn covers_dir(&self) -> PathBuf {
        self.config.data_dir.join("covers")
    }

    pub fn fonts_dir(&self) -> PathBuf {
        self.config.data_dir.join("fonts")
    }

    /// Per-book annotations store: `data/annotations/{book_id}.toml`
    pub fn annotations_store(&self, book_id: &str) -> Store<AnnotationsData> {
        Store::new(
            self.config
                .data_dir
                .join("annotations")
                .join(format!("{book_id}.toml")),
        )
    }
}
