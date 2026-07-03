use chrono::{DateTime, Duration, Utc};

pub const AUTO_BACKUP_INTERVAL_HOURS: i64 = 24;

pub fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

pub fn should_run_auto_backup(last_backup_at: Option<String>) -> bool {
    let Some(last_backup_at) = last_backup_at else {
        return true;
    };
    DateTime::parse_from_rfc3339(&last_backup_at)
        .map(|last| {
            Utc::now().signed_duration_since(last.with_timezone(&Utc))
                >= Duration::hours(AUTO_BACKUP_INTERVAL_HOURS)
        })
        .unwrap_or(true)
}

pub fn round_money(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
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
