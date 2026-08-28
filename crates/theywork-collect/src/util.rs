use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;
use theywork_core::Millis;

/// Convert the path spellings used by Windows and WSL into one stable office id.
pub fn normalize_office_path(input: &str) -> String {
    let input = input.trim();
    if input.is_empty() {
        return String::new();
    }

    let slashed = input.replace('\\', "/");
    let wsl_unc = is_wsl_unc(&slashed);
    let windows_shaped = wsl_unc || is_windows_drive(input) || is_windows_mount(&slashed);
    let absolute = slashed.starts_with('/') || wsl_unc;
    let mut components = Vec::new();
    let mut unc_components_to_skip = if wsl_unc { 2 } else { 0 };

    for component in slashed.split('/') {
        if wsl_unc && !component.is_empty() && unc_components_to_skip > 0 {
            // The UNC server and distro are transport details, not folders in
            // the Linux project path.
            unc_components_to_skip -= 1;
            continue;
        }
        if component.is_empty() || component == "." {
            continue;
        }
        if component == ".." {
            if components.last().is_some_and(|last| *last != "..") {
                components.pop();
            } else if !absolute {
                components.push(component);
            }
            continue;
        }
        components.push(component);
    }

    let mut normalized = String::new();
    if absolute {
        normalized.push('/');
    }
    normalized.push_str(&components.join("/"));
    if normalized.len() > 1 {
        normalized = normalized.trim_end_matches('/').to_string();
    }

    if windows_shaped {
        normalized.to_lowercase()
    } else {
        normalized
    }
}

fn is_wsl_unc(path: &str) -> bool {
    path.split('/')
        .find(|component| !component.is_empty())
        .is_some_and(|host| host.eq_ignore_ascii_case("wsl.localhost"))
}

fn is_windows_drive(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 3 && bytes[1] == b':' && (bytes[2] == b'/' || bytes[2] == b'\\')
}

fn is_windows_mount(path: &str) -> bool {
    let mut components = path.split('/').filter(|component| !component.is_empty());
    components
        .next()
        .is_some_and(|mount| mount.eq_ignore_ascii_case("mnt"))
        && components
            .next()
            .is_some_and(|drive| drive.len() == 1 && drive.as_bytes()[0].is_ascii_alphabetic())
}
pub(crate) const DETAIL_LIMIT: usize = 120;

/// Keep details useful in a desk caption without allowing an agent-controlled
/// command or message to make the UI (or a collector's state) unbounded.
pub(crate) fn truncate_detail(input: &str) -> String {
    let mut output = String::with_capacity(input.len().min(DETAIL_LIMIT));
    let mut count = 0;
    let mut pending_space = false;

    for ch in input.chars() {
        if ch.is_whitespace() {
            if !output.is_empty() {
                pending_space = true;
            }
            continue;
        }

        if pending_space {
            if count == DETAIL_LIMIT {
                break;
            }
            output.push(' ');
            count += 1;
            pending_space = false;
        }

        if count == DETAIL_LIMIT {
            break;
        }
        output.push(ch);
        count += 1;
    }

    output
}

pub(crate) fn short_id(id: &str) -> String {
    let shortened: String = id.chars().take(8).collect();
    if shortened.is_empty() {
        "worker".to_string()
    } else {
        shortened
    }
}

pub(crate) fn path_allowed(path: &str, prefixes: &[PathBuf]) -> bool {
    if prefixes.is_empty() {
        return true;
    }

    let path = normalize_office_path(path);
    prefixes.iter().any(|prefix| {
        let prefix = normalize_office_path(&prefix.to_string_lossy());
        !prefix.is_empty() && Path::new(&path).starts_with(Path::new(&prefix))
    })
}

pub(crate) fn timestamp_value(value: Option<&Value>) -> Option<Millis> {
    match value {
        Some(Value::Number(number)) => number.as_i64(),
        Some(Value::String(value)) => parse_rfc3339(value),
        _ => None,
    }
}

fn parse_rfc3339(value: &str) -> Option<Millis> {
    let (date, clock) = value.split_once('T').or_else(|| value.split_once(' '))?;
    let mut date_parts = date.split('-');
    let year = date_parts.next()?.parse::<i64>().ok()?;
    let month = date_parts.next()?.parse::<i64>().ok()?;
    let day = date_parts.next()?.parse::<i64>().ok()?;
    if date_parts.next().is_some() || !(1..=12).contains(&month) || day < 1 {
        return None;
    }

    let clock_bytes = clock.as_bytes();
    let zone_index = clock_bytes
        .iter()
        .enumerate()
        .skip(2)
        .find(|(_, byte)| matches!(byte, b'Z' | b'z' | b'+' | b'-'))
        .map(|(index, _)| index);

    let (time, zone) = match zone_index {
        Some(index) => (&clock[..index], &clock[index..]),
        None => return None,
    };
    let mut time_parts = time.split(':');
    let hour = time_parts.next()?.parse::<i64>().ok()?;
    let minute = time_parts.next()?.parse::<i64>().ok()?;
    let second_and_fraction = time_parts.next()?;
    if time_parts.next().is_some() || hour > 23 || minute > 59 {
        return None;
    }

    let (second_text, fraction_text) = match second_and_fraction.split_once('.') {
        Some((second, fraction)) => (second, Some(fraction)),
        None => (second_and_fraction, None),
    };
    let second = second_text.parse::<i64>().ok()?;
    if !(0..=60).contains(&second) {
        return None;
    }
    let millis = fraction_text
        .map(parse_fraction_millis)
        .unwrap_or(Some(0))?;

    let offset_minutes = match zone {
        "Z" | "z" => 0,
        zone if zone.len() == 6 && zone.as_bytes()[3] == b':' => {
            let sign = match zone.as_bytes()[0] {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let hours = zone[1..3].parse::<i64>().ok()?;
            let minutes = zone[4..6].parse::<i64>().ok()?;
            if hours > 23 || minutes > 59 {
                return None;
            }
            sign * (hours * 60 + minutes)
        }
        zone if zone.len() == 5 => {
            let sign = match zone.as_bytes()[0] {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let hours = zone[1..3].parse::<i64>().ok()?;
            let minutes = zone[3..5].parse::<i64>().ok()?;
            if hours > 23 || minutes > 59 {
                return None;
            }
            sign * (hours * 60 + minutes)
        }
        _ => return None,
    };

    let days = days_from_civil(year, month, day);
    let seconds = days
        .checked_mul(86_400)?
        .checked_add(hour.checked_mul(3_600)?)?
        .checked_add(minute.checked_mul(60)?)?
        .checked_add(second.min(59))?
        .checked_sub(offset_minutes.checked_mul(60)?)?;
    seconds.checked_mul(1_000)?.checked_add(millis)
}

fn parse_fraction_millis(fraction: &str) -> Option<i64> {
    if fraction.is_empty() || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let mut millis = 0;
    for (index, byte) in fraction.bytes().take(3).enumerate() {
        millis += i64::from(byte - b'0')
            * match index {
                0 => 100,
                1 => 10,
                _ => 1,
            };
    }
    Some(millis)
}

// Gregorian calendar days relative to 1970-01-01. Keeping this here avoids
// making the collectors depend on a date/time crate just to parse transcript
// timestamps.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let adjusted_year = year - if month <= 2 { 1 } else { 0 };
    let era = if adjusted_year >= 0 {
        adjusted_year / 400
    } else {
        (adjusted_year - 399) / 400
    };
    let year_of_era = adjusted_year - era * 400;
    let month_prime = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month_prime + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Resolve a working directory to the nearest enclosing Git root once per
/// normalized spelling.
pub(crate) fn repository_root(input: &str, cache: &mut HashMap<String, String>) -> String {
    let normalized = normalize_office_path(input);
    if normalized.is_empty() {
        return normalized;
    }
    if let Some(cached) = cache.get(&normalized) {
        return cached.clone();
    }

    let mut current = PathBuf::from(filesystem_path(input));
    let result = loop {
        if current.join(".git").exists() {
            break normalize_office_path(&current.to_string_lossy());
        }
        if !current.pop() {
            break normalized.clone();
        }
    };
    cache.insert(normalized, result.clone());
    result
}

fn filesystem_path(input: &str) -> String {
    let slashed = input.trim().replace('\\', "/");
    if !is_wsl_unc(&slashed) {
        return slashed.trim_end_matches('/').to_string();
    }

    let mut components = slashed.split('/').filter(|component| !component.is_empty());
    components.next();
    components.next();
    let rest = components.collect::<Vec<_>>().join("/");
    if rest.is_empty() {
        "/".to_string()
    } else {
        format!("/{rest}")
    }
}

pub(crate) fn recency_cutoff(now: Millis, active_within: Duration) -> Millis {
    let duration_ms = active_within.as_millis();
    if duration_ms > i64::MAX as u128 {
        i64::MIN
    } else {
        now.saturating_sub(duration_ms as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_caps_details() {
        let detail = truncate_detail("  one\n two\tthree  ");
        assert_eq!(detail, "one two three");

        let long = "x".repeat(DETAIL_LIMIT + 10);
        assert_eq!(truncate_detail(&long).chars().count(), DETAIL_LIMIT);
    }

    #[test]
    fn normalizes_real_office_path_spellings() {
        assert_eq!(
            normalize_office_path("/home/gc/AIStudio/projects/hugo-ai"),
            "/home/gc/AIStudio/projects/hugo-ai"
        );
        assert_eq!(
            normalize_office_path(
                r"\\wsl.localhost\Ubuntu-22.04\home\gc\AIStudio\projects\hugo-ai"
            ),
            "/home/gc/aistudio/projects/hugo-ai"
        );
        assert_eq!(
            normalize_office_path("/mnt/c/users/pc/onedrive/documentos/aistudio 2"),
            "/mnt/c/users/pc/onedrive/documentos/aistudio 2"
        );
    }

    #[test]
    fn parses_utc_timestamp_to_epoch_millis() {
        let value = Value::String("2026-08-27T17:38:22.306Z".into());
        assert_eq!(timestamp_value(Some(&value)), Some(1_787_852_302_306));
    }
}
