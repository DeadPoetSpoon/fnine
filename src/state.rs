use crate::cache::Cache;
use crate::config::Config;
use crate::db::annotations::AnnotationsData;
use crate::db::books::{Book, BooksData};
use crate::db::fonts::FontsData;
use crate::db::progress::ProgressData;
use crate::db::settings::Settings;
use crate::db::store::Store;
use crate::epub::parser::{ChapterContent, ChapterEntry};
use crate::error::AppError;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub books: Arc<Store<BooksData>>,
    pub progress: Arc<Store<ProgressData>>,
    pub settings: Arc<Store<Settings>>,
    pub fonts: Arc<Store<FontsData>>,
    pub coordinator: Arc<tokio::sync::Mutex<()>>,
    pub parsers: Arc<tokio::sync::Semaphore>,
    pub chapter_cache: Cache<String, Arc<ChapterContent>>,
    pub toc_cache: Cache<String, Vec<ChapterEntry>>,
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
            fonts: Arc::new(Store::new(data.join("fonts.toml"))),
            coordinator: Arc::new(tokio::sync::Mutex::new(())),
            parsers: Arc::new(tokio::sync::Semaphore::new(2)),
            chapter_cache: Cache::new(),
            toc_cache: Cache::with_capacity(4 * 1024 * 1024),
            books_cache: Cache::new(),
            progress_cache: Cache::new(),
            settings_cache: Cache::new(),
            config,
        }
    }

    pub fn books_dir(&self) -> PathBuf {
        self.config.data_dir.join("books")
    }

    pub async fn initialize(&self) -> Result<std::fs::File, AppError> {
        tokio::fs::create_dir_all(&self.config.data_dir).await?;
        let lock_path = self.config.data_dir.join(".fnine.lock");
        if tokio::fs::symlink_metadata(&lock_path)
            .await
            .is_ok_and(|meta| meta.file_type().is_symlink())
        {
            return Err(AppError::Internal("Data lock cannot be a symlink".into()));
        }
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)?;
        lock.try_lock().map_err(|error| {
            AppError::Internal(format!("Data directory is already locked: {error}"))
        })?;
        for name in [
            "books",
            "covers",
            "fonts",
            "annotations",
            "staging",
            "imports",
            "deletions",
            "backups",
        ] {
            let path = self.config.data_dir.join(name);
            if tokio::fs::symlink_metadata(&path)
                .await
                .is_ok_and(|meta| meta.file_type().is_symlink())
            {
                return Err(AppError::Internal(format!(
                    "Data subdirectory {name} cannot be a symlink"
                )));
            }
            tokio::fs::create_dir_all(path).await?;
        }
        self.books.load().await?;
        self.progress.load().await?;
        self.settings.load().await?;
        self.fonts.load().await?;
        let mut imports = tokio::fs::read_dir(self.config.data_dir.join("imports")).await?;
        while let Some(entry) = imports.next_entry().await? {
            let path = entry.path();
            if let Some(id) = path.file_stem().and_then(|stem| stem.to_str()) {
                crate::validation::id(id)?;
                if !self
                    .books
                    .load()
                    .await?
                    .books
                    .iter()
                    .any(|book| book.id == id)
                {
                    remove_if_exists(&self.books_dir().join(format!("{id}.epub"))).await?;
                    for ext in ["jpg", "png", "gif", "webp"] {
                        remove_if_exists(&self.covers_dir().join(format!("{id}.{ext}"))).await?;
                    }
                } else if !self.books_dir().join(format!("{id}.epub")).is_file() {
                    return Err(AppError::Internal(format!(
                        "Committed import {id} is missing its EPUB"
                    )));
                }
                remove_if_exists(&path).await?;
            }
        }
        let mut staged = tokio::fs::read_dir(self.config.data_dir.join("staging")).await?;
        while let Some(entry) = staged.next_entry().await? {
            if entry.file_type().await?.is_file() {
                tokio::fs::remove_file(entry.path()).await?;
            }
        }
        let mut deletions = tokio::fs::read_dir(self.config.data_dir.join("deletions")).await?;
        while let Some(entry) = deletions.next_entry().await? {
            if let Some(id) = entry.path().file_stem().and_then(|stem| stem.to_str()) {
                crate::validation::id(id)?;
                self.finish_delete(id).await?;
            }
        }
        self.migrate_fonts().await?;
        Ok(lock)
    }

    async fn migrate_fonts(&self) -> Result<(), AppError> {
        let mut inventory = self.fonts.load().await?;
        let mut settings = self.settings.load().await?;
        let mut entries = tokio::fs::read_dir(self.fonts_dir()).await?;
        while let Some(entry) = entries.next_entry().await? {
            if !entry.file_type().await?.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if inventory.fonts.iter().any(|font| font.id == name) {
                continue;
            }
            let extension = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
            if !["ttf", "woff2"].contains(&extension.as_str()) {
                continue;
            }
            let id = format!("{}.{}", uuid::Uuid::new_v4(), extension);
            tokio::fs::copy(entry.path(), self.fonts_dir().join(&id)).await?;
            inventory.fonts.push(crate::db::fonts::Font {
                id: id.clone(),
                name: name.clone(),
            });
            self.fonts.save(&inventory).await?;
            if settings.font_family == name {
                settings.font_family = id;
                self.settings.save(&settings).await?;
            }
            tokio::fs::remove_file(entry.path()).await?;
        }
        Ok(())
    }

    pub async fn book(&self, id: &str) -> Result<Book, AppError> {
        crate::validation::id(id)?;
        self.load_books()
            .await?
            .into_iter()
            .find(|book| book.id == id)
            .ok_or_else(|| AppError::NotFound("Book not found".into()))
    }

    pub async fn chapter(
        &self,
        book_id: &str,
        index: usize,
    ) -> Result<Arc<ChapterContent>, AppError> {
        let key = format!("{book_id}:ch:{index}");
        if let Some(chapter) = self.chapter_cache.get(&key) {
            return Ok(chapter);
        }
        let path = self.books_dir().join(format!("{book_id}.epub"));
        let permit = self
            .parsers
            .clone()
            .acquire_owned()
            .await
            .map_err(|error| AppError::Internal(error.to_string()))?;
        let chapter = Arc::new(
            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                crate::epub::parser::read_chapter(&path, index)
            })
            .await
            .map_err(|error| AppError::Internal(error.to_string()))??,
        );
        self.chapter_cache
            .insert_weighted(key, chapter.clone(), chapter.weight());
        Ok(chapter)
    }

    pub async fn finish_delete(&self, id: &str) -> Result<(), AppError> {
        let mut progress = self.progress.load().await?;
        progress.entries.remove(id);
        self.progress.save(&progress).await?;
        remove_if_exists(
            &self
                .config
                .data_dir
                .join("annotations")
                .join(format!("{id}.toml")),
        )
        .await?;
        remove_if_exists(&self.books_dir().join(format!("{id}.epub"))).await?;
        for ext in ["jpg", "png", "gif", "webp"] {
            remove_if_exists(&self.covers_dir().join(format!("{id}.{ext}"))).await?;
        }
        let mut books = self.books.load().await?;
        books.books.retain(|book| book.id != id);
        self.books.save(&books).await?;
        self.invalidate_books_cache();
        self.invalidate_progress_cache();
        self.chapter_cache
            .invalidate_where(|key| key.starts_with(&format!("{id}:")));
        self.toc_cache.invalidate(&id.to_string());
        remove_if_exists(
            &self
                .config
                .data_dir
                .join("deletions")
                .join(format!("{id}.toml")),
        )
        .await?;
        Ok(())
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

pub async fn remove_if_exists(path: &std::path::Path) -> Result<(), AppError> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}
