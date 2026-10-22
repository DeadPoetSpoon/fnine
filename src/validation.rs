use crate::error::AppError;

pub fn id(value: &str) -> Result<(), AppError> {
    if uuid::Uuid::parse_str(value).is_err() || value.len() != 36 {
        return Err(AppError::BadRequest("Invalid identifier".into()));
    }
    Ok(())
}

pub fn position(value: f64) -> Result<(), AppError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(AppError::BadRequest(
            "Position must be finite and between 0 and 1".into(),
        ));
    }
    Ok(())
}

pub fn color(value: &str) -> Result<(), AppError> {
    if !["#ffee77", "#a8e6a3", "#a6d8ff", "#ffb3c6"].contains(&value) {
        return Err(AppError::BadRequest("Unsupported highlight color".into()));
    }
    Ok(())
}

pub fn text(value: &str) -> Result<(), AppError> {
    if value.len() > 16 * 1024 {
        return Err(AppError::TooLarge);
    }
    Ok(())
}
