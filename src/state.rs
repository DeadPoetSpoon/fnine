use crate::cache::Cache;
use crate::config::Config;
use crate::db::annotations::AnnotationsData;
use crate::db::books::{Book, BooksData};
use crate::db::progress::ProgressData;
use crate::db::settings::Settings;
use crate::db::store::Store;
use crate::error::AppError;
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
    pub progress_cache: Cache<(), ProgressData>,
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
            progress_cache: Cache::new(),
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

    // ── Cache-first read helpers ───────────────────────────────

    /// Load books list (cache-first).
    pub async fn load_books(&self) -> Result<Vec<Book>, AppError> {
        if let Some(cached) = self.books_cache.get(&()) {
            return Ok(cached);
        }
        let data = self.books.load().await?;
        self.books_cache.insert((), data.books.clone());
        Ok(data.books)
    }

    /// Load reading progress (cache-first).
    pub async fn load_progress(&self) -> Result<ProgressData, AppError> {
        if let Some(cached) = self.progress_cache.get(&()) {
            return Ok(cached);
        }
        let data = self.progress.load().await?;
        self.progress_cache.insert((), data.clone());
        Ok(data)
    }

    /// Load user settings (cache-first).
    pub async fn load_settings(&self) -> Result<Settings, AppError> {
        if let Some(cached) = self.settings_cache.get(&()) {
            return Ok(cached);
        }
        let s = self.settings.load().await?;
        self.settings_cache.insert((), s.clone());
        Ok(s)
    }

    // ── Cache invalidation helpers ─────────────────────────────

    /// Invalidate books cache (call after mutation).
    pub fn invalidate_books_cache(&self) {
        self.books_cache.invalidate(&());
    }

    /// Invalidate progress cache (call after mutation).
    pub fn invalidate_progress_cache(&self) {
        self.progress_cache.invalidate(&());
    }

    /// Invalidate settings cache (call after mutation).
    pub fn invalidate_settings_cache(&self) {
        self.settings_cache.invalidate(&());
    }
}
