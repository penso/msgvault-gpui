//! Display formatting: dates, sizes, counts and label colors.

use {
    chrono::{DateTime, Datelike, Local, Utc},
    gpui_kit::{Hsla, rgb},
};

/// Compact list timestamp: time today, weekday this week, date otherwise.
pub(crate) fn list_date(time: DateTime<Utc>, now: DateTime<Local>) -> String {
    let local = time.with_timezone(&Local);
    let days = now
        .date_naive()
        .signed_duration_since(local.date_naive())
        .num_days();
    if days == 0 {
        local.format("%-I:%M %p").to_string()
    } else if days == 1 {
        "Yesterday".into()
    } else if (2..7).contains(&days) {
        local.format("%A").to_string()
    } else if local.year() == now.year() {
        local.format("%b %-d").to_string()
    } else {
        local.format("%b %-d, %Y").to_string()
    }
}

/// Reader timestamp, e.g. `Tue, Mar 5, 2024 at 9:41 AM`.
pub(crate) fn full_date(time: DateTime<Utc>) -> String {
    time.with_timezone(&Local)
        .format("%a, %b %-d, %Y at %-I:%M %p")
        .to_string()
}

pub(crate) fn bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{n} B")
    } else if value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// `12345` as `12,345`.
pub(crate) fn count(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

pub(crate) fn plural(n: u64, one: &str, many: &str) -> String {
    format!(
        "{} {}",
        count(n),
        if n == 1 {
            one
        } else {
            many
        }
    )
}

pub(crate) fn hex_color(raw: &str) -> Option<Hsla> {
    let hex = raw.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    u32::from_str_radix(hex, 16)
        .ok()
        .map(|v| Hsla::from(rgb(v)))
}

/// A stable, pleasant color for labels without one.
pub(crate) fn label_color(name: &str, stored: Option<&str>) -> Hsla {
    if let Some(color) = stored.and_then(hex_color) {
        return color;
    }
    const PALETTE: [u32; 10] = [
        0x4a86e8, 0x8e63ce, 0x16a766, 0xf2a60c, 0xe66550, 0x2da2bb, 0xd5608c, 0x3dc789, 0x7a8cff,
        0xc28b3c,
    ];
    let hash = name.bytes().fold(2166136261u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(16777619)
    });
    rgb(PALETTE[(hash % PALETTE.len() as u32) as usize]).into()
}

/// Filenames from mail headers are untrusted: keep only a safe final component.
pub(crate) fn safe_filename(raw: &str, fallback: &str) -> String {
    let name: String = raw
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let name = name.trim().trim_start_matches('.').to_owned();
    if name.is_empty() {
        fallback.to_owned()
    } else {
        name
    }
}

#[cfg(test)]
mod tests {
    use {super::*, chrono::TimeZone};

    #[test]
    fn counts_and_sizes() {
        assert_eq!(count(0), "0");
        assert_eq!(count(1234567), "1,234,567");
        assert_eq!(bytes(512), "512 B");
        assert_eq!(bytes(1536), "1.5 KB");
        assert_eq!(bytes(300 * 1024 * 1024), "300 MB");
        assert_eq!(plural(1, "message", "messages"), "1 message");
    }

    #[test]
    fn list_dates_are_relative() {
        let now = Local.with_ymd_and_hms(2024, 6, 14, 18, 0, 0).single();
        let Some(now) = now else {
            return;
        };
        let at = |days: i64| (now - chrono::Duration::days(days)).with_timezone(&Utc);
        assert!(list_date(at(0), now).ends_with("PM"));
        assert_eq!(list_date(at(1), now), "Yesterday");
        assert_eq!(list_date(at(3), now), "Tuesday");
        assert_eq!(list_date(at(40), now), "May 5");
        assert_eq!(list_date(at(400), now), "May 11, 2023");
    }

    #[test]
    fn filenames_cannot_escape() {
        assert_eq!(safe_filename("../../etc/passwd", "f"), "passwd");
        assert_eq!(safe_filename("..", "f"), "f");
        assert_eq!(safe_filename("a:b?.pdf", "f"), "a_b_.pdf");
        assert_eq!(safe_filename("", "attachment"), "attachment");
    }

    #[test]
    fn colors_parse_and_fall_back() {
        assert!(hex_color("#4a86e8").is_some());
        assert!(hex_color("blue").is_none());
        assert_eq!(label_color("Work", None), label_color("Work", Some("nope")));
    }
}
