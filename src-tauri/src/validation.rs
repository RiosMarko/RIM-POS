pub fn validate_pin(pin: &str, min_len: usize, label: &str) -> Result<(), String> {
    let trimmed = pin.trim();
    if trimmed.len() < min_len {
        return Err(format!("{label} minimo de {min_len} digitos"));
    }
    if trimmed.len() > 12 {
        return Err(format!("{label} maximo de 12 digitos"));
    }
    if !trimmed.chars().all(|char| char.is_ascii_digit()) {
        return Err(format!("{label} solo acepta digitos"));
    }
    Ok(())
}

pub fn validate_required_text(value: &str, min_len: usize, message: &str) -> Result<(), String> {
    if value.trim().len() < min_len {
        return Err(message.to_string());
    }
    Ok(())
}

pub fn validate_non_negative(value: f64, message: &str) -> Result<(), String> {
    if !value.is_finite() || value < 0.0 {
        return Err(message.to_string());
    }
    Ok(())
}

pub fn validate_positive(value: f64, message: &str) -> Result<(), String> {
    if !value.is_finite() || value <= 0.0 {
        return Err(message.to_string());
    }
    Ok(())
}

pub fn validate_optional_email(value: Option<&str>) -> Result<(), String> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };
    let has_at = value.contains('@');
    let has_dot_after_at = value
        .split_once('@')
        .map(|(_, domain)| domain.contains('.'))
        .unwrap_or(false);
    if has_at && has_dot_after_at {
        Ok(())
    } else {
        Err("Email invalido".to_string())
    }
}

pub fn validate_optional_rfc(value: Option<&str>) -> Result<(), String> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };
    let len = value.len();
    if (12..=13).contains(&len) && value.chars().all(|char| char.is_ascii_alphanumeric()) {
        Ok(())
    } else {
        Err("RFC invalido".to_string())
    }
}
