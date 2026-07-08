use chrono::{DateTime, Local, Utc};

pub fn today_local_key() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

pub fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

pub fn should_run_auto_backup(last_backup_at: Option<String>) -> bool {
    let Some(last_backup_at) = last_backup_at else {
        return true;
    };
    DateTime::parse_from_rfc3339(&last_backup_at)
        .map(|last| last.with_timezone(&Local).format("%Y-%m-%d").to_string() != today_local_key())
        .unwrap_or(true)
}

pub fn round_money(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// Formatea una cantidad quitando ceros decimales sobrantes.
/// 2.0 -> "2", 0.5 -> "0.5", 0.781 -> "0.781", 0.780 -> "0.78".
pub fn format_quantity(value: f64) -> String {
    let text = format!("{value:.3}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn period_key(created_at: &str) -> Result<String, String> {
    created_at
        .get(0..7)
        .filter(|value| value.len() == 7 && value.as_bytes().get(4) == Some(&b'-'))
        .map(str::to_string)
        .ok_or_else(|| "Fecha invalida para folio mensual".to_string())
}

pub fn next_monthly_seq(current_max: i64) -> i64 {
    current_max + 1
}

// Legacy folio format (pre global-consecutive); kept for reference and tests.
#[allow(dead_code)]
pub fn visible_monthly_folio(period: &str, monthly_seq: i64) -> String {
    format!("{period}-{monthly_seq:03}")
}

pub fn average_ticket(total: f64, tickets: i64) -> f64 {
    if tickets > 0 {
        round_money(total / tickets as f64)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::{should_run_auto_backup, today_local_key};
    use chrono::{Duration, Local, Utc};

    #[test]
    fn format_quantity_trims_trailing_zeros() {
        use super::format_quantity;
        assert_eq!(format_quantity(2.0), "2");
        assert_eq!(format_quantity(3.0), "3");
        assert_eq!(format_quantity(0.5), "0.5");
        assert_eq!(format_quantity(0.781), "0.781");
        assert_eq!(format_quantity(0.78), "0.78");
        assert_eq!(format_quantity(0.0), "0");
    }

    #[test]
    fn auto_backup_runs_when_no_previous_backup() {
        assert!(should_run_auto_backup(None));
    }

    #[test]
    fn auto_backup_skips_when_backup_already_created_today() {
        let now = Utc::now().to_rfc3339();
        assert_eq!(
            should_run_auto_backup(Some(now)),
            false,
            "same local day {} should not create backup again",
            today_local_key()
        );
    }

    #[test]
    fn auto_backup_runs_when_last_backup_is_previous_local_day() {
        let yesterday = (Local::now() - Duration::days(1))
            .with_timezone(&Utc)
            .to_rfc3339();
        assert!(should_run_auto_backup(Some(yesterday)));
    }
}
