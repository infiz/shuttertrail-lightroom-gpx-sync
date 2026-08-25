use chrono::{DateTime, Duration, FixedOffset, NaiveDateTime, SecondsFormat, TimeZone, Utc};

pub fn normalize_offset(value: &str) -> Result<String, String> {
    let seconds = parse_offset_seconds(value)?;
    let sign = if seconds < 0 { '-' } else { '+' };
    let absolute = seconds.unsigned_abs();
    Ok(format!(
        "{sign}{:02}:{:02}",
        absolute / 3600,
        absolute % 3600 / 60
    ))
}

pub fn photo_time_to_utc(
    value: &str,
    subsecond: Option<&str>,
    offset: &str,
) -> Result<DateTime<Utc>, String> {
    let timestamp = value
        .get(..19)
        .ok_or_else(|| format!("Unsupported capture timestamp: {value}"))?;
    let naive = NaiveDateTime::parse_from_str(timestamp, "%Y:%m:%d %H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S"))
        .map_err(|_| format!("Unsupported capture timestamp: {value}"))?;
    let digits: String = subsecond
        .unwrap_or_default()
        .chars()
        .take_while(char::is_ascii_digit)
        .take(9)
        .collect();
    let nanos = if digits.is_empty() {
        0
    } else {
        format!("{digits:0<9}").parse::<i64>().unwrap_or(0)
    };
    let naive = naive + Duration::nanoseconds(nanos);
    let seconds = parse_offset_seconds(offset)?;
    let zone = FixedOffset::east_opt(seconds).ok_or_else(|| "Invalid UTC offset".to_string())?;
    let local = zone
        .from_local_datetime(&naive)
        .single()
        .ok_or_else(|| "Capture timestamp is ambiguous".to_string())?;
    Ok(local.with_timezone(&Utc))
}

pub fn display_utc(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn parse_offset_seconds(value: &str) -> Result<i32, String> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("z") {
        return Ok(0);
    }
    let bytes = value.as_bytes();
    if bytes.len() != 6 && bytes.len() != 5 {
        return Err("UTC offset must look like -07:00 or +05:30".into());
    }
    let sign = match bytes[0] {
        b'+' => 1,
        b'-' => -1,
        _ => return Err("UTC offset must begin with + or -".into()),
    };
    let normalized = value.replace(':', "");
    let hour = normalized[1..3]
        .parse::<i32>()
        .map_err(|_| "Invalid UTC offset hour".to_string())?;
    let minute = normalized[3..5]
        .parse::<i32>()
        .map_err(|_| "Invalid UTC offset minute".to_string())?;
    if hour > 14 || minute > 59 || (hour == 14 && minute != 0) {
        return Err("UTC offset is outside the valid range".into());
    }
    Ok(sign * (hour * 3600 + minute * 60))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_photo_time_without_using_computer_timezone() {
        let utc = photo_time_to_utc("2026:07:21 12:00:00", Some("25"), "-07:00").unwrap();
        assert_eq!(display_utc(utc), "2026-07-21T19:00:00.250Z");
    }

    #[test]
    fn normalizes_offsets() {
        assert_eq!(normalize_offset("+0530").unwrap(), "+05:30");
        assert!(normalize_offset("+15:00").is_err());
    }
}
