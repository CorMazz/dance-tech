//! Stripe show-window metadata for Check In products.
//!
//! Missing both interval tags means the product is always visible. A bad parse hides the product.

use crate::check_in::metadata::STRIPE_KEYS;
use chrono::{DateTime, Datelike, Duration, NaiveDateTime, NaiveTime, TimeZone, Utc, Weekday};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

const DEFAULT_TZ: &str = "America/New_York";

/// When a catalog product may appear on Check In.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ShowSchedule {
    #[serde(default)]
    windows: Vec<AbsoluteWindow>,
    #[serde(default)]
    weekly: Vec<WeeklyWindow>,
    #[serde(default)]
    timezone: String,
    /// Set when Stripe metadata could not be parsed. Product is hidden.
    #[serde(default)]
    pub parse_error: Option<String>,
    #[serde(default)]
    raw_timezone: Option<String>,
    #[serde(default)]
    raw_interval: Option<String>,
    #[serde(default)]
    raw_weekly: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct AbsoluteWindow {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct WeeklyWindow {
    /// Monday = 0 … Sunday = 6
    weekday: u8,
    start: NaiveTime,
    end: NaiveTime,
}

impl ShowSchedule {
    fn always() -> Self {
        Self::default()
    }

    fn tz(&self) -> Tz {
        Tz::from_str(&self.timezone).unwrap_or(chrono_tz::America::New_York)
    }

    /// True when the product should be sold right now.
    pub fn is_visible(&self, now: DateTime<Utc>) -> bool {
        if self.parse_error.is_some() {
            return false;
        }
        if self.windows.is_empty() && self.weekly.is_empty() {
            return true;
        }
        let in_absolute = self
            .windows
            .iter()
            .any(|window| now >= window.start && now <= window.end);
        if in_absolute {
            return true;
        }
        let local = now.with_timezone(&self.tz());
        self.weekly.iter().any(|window| window.contains(local))
    }

    /// Short admin label: Live, Hidden until …, or parse error.
    pub fn status_label(&self, now: DateTime<Utc>) -> String {
        if let Some(err) = &self.parse_error {
            return format!("Hidden (parse error: {err})");
        }
        if self.is_visible(now) {
            return "Live".to_string();
        }
        self.next_start(now).map_or_else(
            || "Hidden".to_string(),
            |start| {
                let local = start.with_timezone(&self.tz());
                format!("Hidden until {}", local.format("%a %H:%M"))
            },
        )
    }

    /// True when no interval tags were set.
    pub fn is_unrestricted(&self) -> bool {
        self.parse_error.is_none() && self.windows.is_empty() && self.weekly.is_empty()
    }

    /// Timezone the windows were interpreted in, plus whether it was the default.
    pub fn timezone_display(&self) -> String {
        if self.is_unrestricted() {
            return "n/a (always visible)".to_string();
        }
        if let Some(err) = &self.parse_error
            && err.starts_with("unknown timezone")
        {
            return self
                .raw_timezone
                .clone()
                .unwrap_or_else(|| "invalid".to_string());
        }
        if self.timezone.is_empty() {
            return DEFAULT_TZ.to_string();
        }
        if self.raw_timezone.is_none() {
            format!("{} (default)", self.timezone)
        } else {
            self.timezone.clone()
        }
    }

    /// One-shot windows in the product timezone, with UTC alongside.
    pub fn interval_summaries(&self) -> Vec<String> {
        let tz = self.tz();
        self.windows
            .iter()
            .map(|window| {
                let start = window.start.with_timezone(&tz);
                let end = window.end.with_timezone(&tz);
                format!(
                    "{} – {} {} (UTC {} – {})",
                    start.format("%a %Y-%m-%d %H:%M"),
                    end.format("%a %Y-%m-%d %H:%M"),
                    tz,
                    window.start.format("%Y-%m-%d %H:%M"),
                    window.end.format("%Y-%m-%d %H:%M")
                )
            })
            .collect()
    }

    /// Recurring weekly windows in the product timezone.
    pub fn weekly_summaries(&self) -> Vec<String> {
        let tz_name = if self.timezone.is_empty() {
            DEFAULT_TZ
        } else {
            self.timezone.as_str()
        };
        self.weekly
            .iter()
            .map(|window| {
                let day = WeeklyWindow::weekday(window.weekday)
                    .map(|day| day.to_string())
                    .unwrap_or_else(|| "?".to_string());
                format!(
                    "{day} {} – {} {tz_name}",
                    window.start.format("%H:%M"),
                    window.end.format("%H:%M")
                )
            })
            .collect()
    }

    /// Raw timezone tag from Stripe, or empty.
    pub fn raw_timezone(&self) -> &str {
        self.raw_timezone.as_deref().unwrap_or("")
    }

    /// Raw interval tag from Stripe, or empty.
    pub fn raw_interval(&self) -> &str {
        self.raw_interval.as_deref().unwrap_or("")
    }

    /// Raw weekly tag from Stripe, or empty.
    pub fn raw_weekly(&self) -> &str {
        self.raw_weekly.as_deref().unwrap_or("")
    }

    fn next_start(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let tz = self.tz();
        let mut soonest: Option<DateTime<Utc>> = None;
        for window in &self.windows {
            if window.start > now {
                soonest = Some(min_dt(soonest, window.start));
            }
        }
        for weekly in &self.weekly {
            if let Some(start) = weekly.next_start(now, tz) {
                soonest = Some(min_dt(soonest, start));
            }
        }
        soonest
    }
}

impl WeeklyWindow {
    fn weekday(self_weekday: u8) -> Option<Weekday> {
        Weekday::try_from(self_weekday).ok()
    }

    fn contains(&self, local: DateTime<Tz>) -> bool {
        let Some(target) = Self::weekday(self.weekday) else {
            return false;
        };
        let time = local.time();
        let day = local.weekday();
        if self.start <= self.end {
            day == target && time >= self.start && time <= self.end
        } else {
            (day == target && time >= self.start) || (day == target.succ() && time <= self.end)
        }
    }

    fn next_start(&self, now: DateTime<Utc>, tz: Tz) -> Option<DateTime<Utc>> {
        let weekday = Self::weekday(self.weekday)?;
        let local = now.with_timezone(&tz);
        let mut date = local.date_naive();
        for _ in 0..8 {
            if date.weekday() == weekday {
                let ndt = date.and_time(self.start);
                if let Some(dt) = tz.from_local_datetime(&ndt).single() {
                    let utc = dt.with_timezone(&Utc);
                    if utc > now {
                        return Some(utc);
                    }
                }
            }
            date += Duration::days(1);
        }
        None
    }
}

fn min_dt(current: Option<DateTime<Utc>>, candidate: DateTime<Utc>) -> DateTime<Utc> {
    match current {
        Some(existing) if existing <= candidate => existing,
        _ => candidate,
    }
}

/// Read show-window tags from Stripe metadata.
pub fn parse_show_schedule(metadata: &HashMap<String, String>) -> ShowSchedule {
    let interval_raw = nonempty(metadata.get(STRIPE_KEYS.show_interval)).map(str::to_string);
    let weekly_raw = nonempty(metadata.get(STRIPE_KEYS.show_weekly)).map(str::to_string);
    let timezone_raw = nonempty(metadata.get(STRIPE_KEYS.show_timezone)).map(str::to_string);
    if interval_raw.is_none() && weekly_raw.is_none() {
        return ShowSchedule::always();
    }

    let tz_name = timezone_raw.as_deref().unwrap_or(DEFAULT_TZ);
    let mut schedule = ShowSchedule {
        timezone: tz_name.to_string(),
        raw_timezone: timezone_raw.clone(),
        raw_interval: interval_raw.clone(),
        raw_weekly: weekly_raw.clone(),
        ..ShowSchedule::default()
    };

    let Ok(tz) = Tz::from_str(tz_name) else {
        schedule.parse_error = Some(format!("unknown timezone {tz_name}"));
        return schedule;
    };

    if let Some(raw) = &interval_raw {
        match parse_intervals(raw, tz) {
            Ok(windows) => schedule.windows = windows,
            Err(err) => {
                schedule.parse_error = Some(err);
                return schedule;
            }
        }
    }
    if let Some(raw) = &weekly_raw {
        match parse_weekly(raw) {
            Ok(weekly) => schedule.weekly = weekly,
            Err(err) => {
                schedule.parse_error = Some(err);
                return schedule;
            }
        }
    }
    schedule
}

fn nonempty(value: Option<&String>) -> Option<&str> {
    value
        .map(String::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn parse_intervals(raw: &str, tz: Tz) -> Result<Vec<AbsoluteWindow>, String> {
    raw.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| parse_one_interval(part, tz))
        .collect()
}

fn parse_one_interval(part: &str, tz: Tz) -> Result<AbsoluteWindow, String> {
    let (left, right) = part
        .split_once('/')
        .ok_or_else(|| format!("interval `{part}` needs start/end separated by /"))?;
    let start_naive = parse_naive_datetime(left.trim())?;
    let end_naive = if right.trim().contains('-') {
        parse_naive_datetime(right.trim())?
    } else {
        let time = parse_naive_time(right.trim())?;
        start_naive.date().and_time(time)
    };
    if end_naive <= start_naive {
        return Err(format!("interval end must be after start: {part}"));
    }
    Ok(AbsoluteWindow {
        start: local_to_utc(tz, start_naive)?,
        end: local_to_utc(tz, end_naive)?,
    })
}

fn parse_weekly(raw: &str) -> Result<Vec<WeeklyWindow>, String> {
    raw.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(parse_one_weekly)
        .collect()
}

fn parse_one_weekly(part: &str) -> Result<WeeklyWindow, String> {
    let (day_str, times) = part
        .split_once(char::is_whitespace)
        .ok_or_else(|| format!("weekly `{part}` needs `Thu 18:00-23:00`"))?;
    let weekday = parse_weekday(day_str.trim())?;
    let (start_str, end_str) = times
        .trim()
        .split_once('-')
        .ok_or_else(|| format!("weekly `{part}` needs a start-end time"))?;
    Ok(WeeklyWindow {
        weekday: u8::try_from(weekday.num_days_from_monday()).unwrap_or(0),
        start: parse_naive_time(start_str.trim())?,
        end: parse_naive_time(end_str.trim())?,
    })
}

fn parse_weekday(raw: &str) -> Result<Weekday, String> {
    match raw.to_ascii_lowercase().as_str() {
        "mon" | "monday" => Ok(Weekday::Mon),
        "tue" | "tues" | "tuesday" => Ok(Weekday::Tue),
        "wed" | "wednesday" => Ok(Weekday::Wed),
        "thu" | "thur" | "thurs" | "thursday" => Ok(Weekday::Thu),
        "fri" | "friday" => Ok(Weekday::Fri),
        "sat" | "saturday" => Ok(Weekday::Sat),
        "sun" | "sunday" => Ok(Weekday::Sun),
        other => Err(format!("unknown weekday {other}")),
    }
}

fn parse_naive_datetime(raw: &str) -> Result<NaiveDateTime, String> {
    NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M")
        .map_err(|_| format!("expected `YYYY-MM-DD HH:MM`, got `{raw}`"))
}

fn parse_naive_time(raw: &str) -> Result<NaiveTime, String> {
    NaiveTime::parse_from_str(raw, "%H:%M").map_err(|_| format!("expected `HH:MM`, got `{raw}`"))
}

fn local_to_utc(tz: Tz, ndt: NaiveDateTime) -> Result<DateTime<Utc>, String> {
    match tz.from_local_datetime(&ndt) {
        chrono::LocalResult::Single(dt) => Ok(dt.with_timezone(&Utc)),
        chrono::LocalResult::Ambiguous(_, _) => {
            Err(format!("{ndt} is ambiguous in {tz} (DST overlap)"))
        }
        chrono::LocalResult::None => Err(format!("{ndt} does not exist in {tz} (DST gap)")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn meta(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    fn ny(year: i32, month: u32, day: u32, hour: u32, min: u32) -> DateTime<Utc> {
        chrono_tz::America::New_York
            .with_ymd_and_hms(year, month, day, hour, min, 0)
            .single()
            .expect("valid NY time")
            .with_timezone(&Utc)
    }

    #[test]
    fn missing_tags_always_visible() {
        let schedule = parse_show_schedule(&HashMap::new());
        assert!(schedule.is_visible(Utc::now()));
        assert_eq!(schedule.status_label(Utc::now()), "Live");
    }

    #[test]
    fn same_day_interval() {
        let schedule = parse_show_schedule(&meta(&[(
            STRIPE_KEYS.show_interval,
            "2026-08-14 18:00/22:00",
        )]));
        assert!(schedule.parse_error.is_none());
        assert!(schedule.is_visible(ny(2026, 8, 14, 18, 0)));
        assert!(schedule.is_visible(ny(2026, 8, 14, 21, 59)));
        assert!(!schedule.is_visible(ny(2026, 8, 14, 17, 59)));
        assert!(!schedule.is_visible(ny(2026, 8, 14, 22, 1)));
    }

    #[test]
    fn multi_day_interval() {
        let schedule = parse_show_schedule(&meta(&[(
            STRIPE_KEYS.show_interval,
            "2026-08-14 22:00/2026-08-15 01:00",
        )]));
        assert!(schedule.is_visible(ny(2026, 8, 14, 23, 30)));
        assert!(schedule.is_visible(ny(2026, 8, 15, 0, 30)));
        assert!(!schedule.is_visible(ny(2026, 8, 15, 1, 1)));
    }

    #[test]
    fn weekly_thursday_edt() {
        let schedule = parse_show_schedule(&meta(&[(STRIPE_KEYS.show_weekly, "Thu 18:00-23:00")]));
        // 2026-08-13 is Thursday, Eastern Daylight Time
        assert!(schedule.is_visible(ny(2026, 8, 13, 18, 0)));
        assert!(schedule.is_visible(ny(2026, 8, 13, 22, 30)));
        assert!(!schedule.is_visible(ny(2026, 8, 13, 17, 59)));
        assert!(!schedule.is_visible(ny(2026, 8, 14, 18, 0)));
    }

    #[test]
    fn weekly_thursday_est() {
        let schedule = parse_show_schedule(&meta(&[(STRIPE_KEYS.show_weekly, "Thu 18:00-23:00")]));
        // 2026-01-15 is Thursday, Eastern Standard Time
        assert!(schedule.is_visible(ny(2026, 1, 15, 18, 0)));
        assert!(!schedule.is_visible(ny(2026, 1, 14, 18, 0)));
    }

    #[test]
    fn interval_or_weekly_union() {
        let schedule = parse_show_schedule(&meta(&[
            (STRIPE_KEYS.show_interval, "2026-08-16 10:00/12:00"),
            (STRIPE_KEYS.show_weekly, "Thu 18:00-23:00"),
        ]));
        assert!(schedule.is_visible(ny(2026, 8, 13, 19, 0)));
        assert!(schedule.is_visible(ny(2026, 8, 16, 11, 0)));
        assert!(!schedule.is_visible(ny(2026, 8, 16, 13, 0)));
    }

    #[test]
    fn bad_interval_fails_closed() {
        let schedule =
            parse_show_schedule(&meta(&[(STRIPE_KEYS.show_interval, "thursday evening")]));
        assert!(schedule.parse_error.is_some());
        assert!(!schedule.is_visible(Utc::now()));
        assert!(
            schedule
                .status_label(Utc::now())
                .starts_with("Hidden (parse error:")
        );
    }

    #[test]
    fn unknown_timezone_fails_closed() {
        let schedule = parse_show_schedule(&meta(&[
            (STRIPE_KEYS.show_interval, "2026-08-14 18:00/22:00"),
            (STRIPE_KEYS.show_timezone, "Mars/Olympus"),
        ]));
        assert!(schedule.parse_error.is_some());
        assert!(!schedule.is_visible(ny(2026, 8, 14, 19, 0)));
    }

    #[test]
    fn dst_gap_fails_closed() {
        let schedule = parse_show_schedule(&meta(&[(
            STRIPE_KEYS.show_interval,
            "2026-03-08 02:30/04:00",
        )]));
        assert!(schedule.parse_error.is_some());
        assert!(!schedule.is_visible(ny(2026, 3, 8, 3, 30)));
    }

    #[test]
    fn hidden_until_uses_next_weekly_start() {
        let schedule = parse_show_schedule(&meta(&[(STRIPE_KEYS.show_weekly, "Thu 18:00-23:00")]));
        let wednesday = ny(2026, 8, 12, 12, 0);
        assert!(!schedule.is_visible(wednesday));
        assert_eq!(schedule.status_label(wednesday), "Hidden until Thu 18:00");
    }

    #[test]
    fn admin_summaries_show_timezone_and_parsed_windows() {
        let schedule = parse_show_schedule(&meta(&[
            (STRIPE_KEYS.show_interval, "2026-08-14 18:00/22:00"),
            (STRIPE_KEYS.show_weekly, "Thu 18:00-23:00"),
        ]));
        assert_eq!(schedule.timezone_display(), "America/New_York (default)");
        let intervals = schedule.interval_summaries();
        assert_eq!(intervals.len(), 1);
        assert!(intervals[0].contains("2026-08-14 18:00"));
        assert!(intervals[0].contains("America/New_York"));
        assert!(intervals[0].contains("UTC"));
        let weeklies = schedule.weekly_summaries();
        assert_eq!(
            weeklies,
            vec!["Thu 18:00 – 23:00 America/New_York".to_string()]
        );
        assert_eq!(schedule.raw_interval(), "2026-08-14 18:00/22:00");
        assert_eq!(schedule.raw_weekly(), "Thu 18:00-23:00");
    }
}
