use crate::error::AppError;
use serde::Serialize;

#[derive(Serialize)]
pub struct Page<Item> {
    pub items: Vec<Item>,
    pub page: usize,
    pub page_size: usize,
    pub total: usize,
    pub pages: usize,
}

impl<Item> Page<Item> {
    pub fn new(items: Vec<Item>, requested: usize) -> Self {
        let total = items.len();
        let page_size = 10;
        let pages = total.div_ceil(page_size).max(1);
        let page = requested.clamp(1, pages);
        Self {
            items: items
                .into_iter()
                .skip((page - 1) * page_size)
                .take(page_size)
                .collect(),
            page,
            page_size,
            total,
            pages,
        }
    }
}

pub fn number(value: Option<&String>) -> Result<usize, AppError> {
    match value {
        None => Ok(1),
        Some(value) => value
            .parse::<usize>()
            .ok()
            .filter(|page| *page > 0)
            .ok_or_else(|| AppError::BadRequest("Invalid page number".into())),
    }
}
