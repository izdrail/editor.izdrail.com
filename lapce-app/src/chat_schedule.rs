//! Scheduled AI chat prompts, stored per workspace in
//! `.lapce/scheduled-prompts.json`. Two shapes: `every_minutes` (interval,
//! clamped) or `at` (once a day at local HH:MM). The scheduler fires one
//! due prompt per minute into the chat lane; everything time-related lives
//! here, pure and tested.

use chrono::{DateTime, Local, NaiveTime};
use serde::{Deserialize, Serialize};

pub const MAX_ENTRIES: usize = 32;
pub const PROMPT_CAP: usize = 4 * 1024;
pub const MIN_INTERVAL_MINUTES: u32 = 5;
/// One week.
pub const MAX_INTERVAL_MINUTES: u32 = 7 * 24 * 60;

pub const EXAMPLE_FILE: &str = r#"{
  "prompts": [
    {
      "prompt": "Summarize what changed in the open file and suggest a commit message",
      "every_minutes": 120
    },
    {
      "prompt": "List the TODO comments in this project",
      "at": "09:00"
    }
  ]
}
"#;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleFile {
    #[serde(default)]
    pub prompts: Vec<ScheduledPrompt>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledPrompt {
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub every_minutes: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    /// Unix seconds of the last fire; written back by the scheduler.
    #[serde(default)]
    pub last_fired_unix: u64,
}

/// Enforce the bounds a hand-edited file can violate: entry cap, prompt
/// cap (char boundary), interval clamp, and dropping entries with no
/// prompt or no schedule.
pub fn normalize(file: &mut ScheduleFile) {
    file.prompts.truncate(MAX_ENTRIES);
    file.prompts.retain_mut(|p| {
        if p.prompt.len() > PROMPT_CAP {
            let mut end = PROMPT_CAP;
            while !p.prompt.is_char_boundary(end) {
                end -= 1;
            }
            p.prompt.truncate(end);
        }
        if p.prompt.trim().is_empty() {
            return false;
        }
        if let Some(m) = p.every_minutes {
            p.every_minutes =
                Some(m.clamp(MIN_INTERVAL_MINUTES, MAX_INTERVAL_MINUTES));
        }
        p.every_minutes.is_some() || p.at.is_some()
    });
}

fn parse_hhmm(at: &str) -> Option<(u32, u32)> {
    let (h, m) = at.split_once(':')?;
    let h: u32 = h.parse().ok()?;
    let m: u32 = m.parse().ok()?;
    (h < 24 && m < 60).then_some((h, m))
}

/// Is this entry due at `now`? Intervals compare against
/// `last_fired_unix`; a daily `at` is due once per local day, after the
/// day's occurrence and only if it has not fired since that occurrence.
pub fn is_due(entry: &ScheduledPrompt, now: &DateTime<Local>) -> bool {
    let now_ts = now.timestamp().max(0) as u64;
    if let Some(mins) = entry.every_minutes {
        let mins =
            mins.clamp(MIN_INTERVAL_MINUTES, MAX_INTERVAL_MINUTES) as u64;
        return now_ts >= entry.last_fired_unix.saturating_add(mins * 60);
    }
    if let Some(at) = &entry.at {
        let Some((h, m)) = parse_hhmm(at) else {
            return false;
        };
        let Some(time) = NaiveTime::from_hms_opt(h, m, 0) else {
            return false;
        };
        let naive = now.date_naive().and_time(time);
        let Some(occ) = naive.and_local_timezone(Local).single() else {
            return false;
        };
        return now >= &occ
            && entry.last_fired_unix < occ.timestamp().max(0) as u64;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Local, NaiveDate, TimeZone};

    fn at(naive: &str) -> DateTime<Local> {
        let naive =
            NaiveDate::parse_from_str(&naive[..10], "%Y-%m-%d")
                .unwrap()
                .and_time(
                    NaiveTime::parse_from_str(&naive[11..], "%H:%M").unwrap(),
                );
        Local.from_local_datetime(&naive).single().unwrap()
    }

    #[test]
    fn example_file_parses_and_normalizes() {
        let mut file: ScheduleFile =
            serde_json::from_str(EXAMPLE_FILE).unwrap();
        normalize(&mut file);
        assert_eq!(file.prompts.len(), 2);
        assert_eq!(file.prompts[0].every_minutes, Some(120));
        assert_eq!(file.prompts[1].at.as_deref(), Some("09:00"));
    }

    #[test]
    fn interval_due_math_respects_last_fire() {
        let entry = ScheduledPrompt {
            prompt: "x".into(),
            every_minutes: Some(30),
            at: None,
            last_fired_unix: 0,
        };
        let now = at("2026-10-17 12:00");
        assert!(is_due(&entry, &now));
        let fired = ScheduledPrompt {
            last_fired_unix: now.timestamp() as u64 - 60,
            ..entry.clone()
        };
        assert!(!is_due(&fired, &now));
    }

    #[test]
    fn interval_is_clamped_into_bounds() {
        let too_fast = ScheduledPrompt {
            prompt: "x".into(),
            every_minutes: Some(1),
            at: None,
            // Fired 4 minutes ago: under the 5-minute floor, not due.
            last_fired_unix: at("2026-10-17 11:56").timestamp() as u64,
        };
        assert!(!is_due(&too_fast, &at("2026-10-17 12:00")));
        let fired_long_ago = ScheduledPrompt {
            last_fired_unix: 0,
            ..too_fast
        };
        assert!(is_due(&fired_long_ago, &at("2026-10-17 12:00")));
    }

    #[test]
    fn daily_at_fires_once_per_day() {
        let entry = ScheduledPrompt {
            prompt: "standup".into(),
            every_minutes: None,
            at: Some("09:00".into()),
            last_fired_unix: 0,
        };
        // Before the occurrence: not due.
        assert!(!is_due(&entry, &at("2026-10-17 08:59")));
        // After the occurrence: due.
        assert!(is_due(&entry, &at("2026-10-17 09:00")));
        // Fired this morning: not due again today.
        let fired = ScheduledPrompt {
            last_fired_unix: at("2026-10-17 09:00").timestamp() as u64,
            ..entry.clone()
        };
        assert!(!is_due(&fired, &at("2026-10-17 23:30")));
        // Yesterday's fire does not block today's occurrence.
        let fired_yesterday = ScheduledPrompt {
            last_fired_unix: at("2026-10-16 09:00").timestamp() as u64,
            ..entry
        };
        assert!(is_due(&fired_yesterday, &at("2026-10-17 09:00")));
    }

    #[test]
    fn invalid_time_is_never_due() {
        for bad in ["9:60", "25:00", "nine", "", "09:00:00"] {
            let entry = ScheduledPrompt {
                prompt: "x".into(),
                every_minutes: None,
                at: Some(bad.into()),
                last_fired_unix: 0,
            };
            assert!(!is_due(&entry, &at("2026-10-17 23:59")), "{bad}");
        }
    }

    #[test]
    fn normalize_drops_clamps_and_clips_safely() {
        let mut file = ScheduleFile {
            prompts: vec![
                ScheduledPrompt {
                    prompt: "   ".into(),
                    every_minutes: Some(60),
                    at: None,
                    last_fired_unix: 0,
                },
                ScheduledPrompt {
                    prompt: "no schedule".into(),
                    every_minutes: None,
                    at: None,
                    last_fired_unix: 0,
                },
                ScheduledPrompt {
                    prompt: "kept 日本語".into(),
                    every_minutes: Some(0),
                    at: None,
                    last_fired_unix: 0,
                },
                ScheduledPrompt {
                    prompt: "日".repeat(PROMPT_CAP),
                    every_minutes: Some(u32::MAX),
                    at: None,
                    last_fired_unix: 0,
                },
            ],
        };
        normalize(&mut file);
        assert_eq!(file.prompts.len(), 2);
        assert_eq!(file.prompts[0].prompt, "kept 日本語");
        assert_eq!(file.prompts[0].every_minutes, Some(MIN_INTERVAL_MINUTES));
        assert_eq!(
            file.prompts[1].every_minutes,
            Some(MAX_INTERVAL_MINUTES)
        );
        assert!(file.prompts[1].prompt.len() <= PROMPT_CAP);
        assert!(std::str::from_utf8(file.prompts[1].prompt.as_bytes()).is_ok());
        // Entry cap.
        let mut big = ScheduleFile {
            prompts: vec![file.prompts[0].clone(); MAX_ENTRIES + 5],
        };
        normalize(&mut big);
        assert_eq!(big.prompts.len(), MAX_ENTRIES);
    }
}
