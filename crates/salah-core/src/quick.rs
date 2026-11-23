//! Quick add: turns one line such as "read 20m after isha daily" into an
//! item draft. Words that are not part of a recognised phrase form the title.

use crate::items::NewItem;
use chrono::{Days, NaiveDate};
use salah_domain::day::BoundaryId;
use salah_domain::error::Error;
use salah_domain::item::{Repeat, Weekdays, validate_title, weekday_index};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Draft {
    pub title: String,
    pub block: Option<BoundaryId>,
    pub repeat: Repeat,
    pub weekdays: Option<Weekdays>,
    pub planned_minutes: Option<i64>,
    /// The tracking day named by "today" or "tomorrow"; `null` when the text
    /// names no day. Set by [`Draft::on`].
    pub start_day: Option<NaiveDate>,
    /// Days after today that the text names, until resolved.
    #[serde(skip)]
    days_ahead: Option<u64>,
}

impl Draft {
    /// Whether the text names a day that [`Draft::on`] must resolve.
    pub fn names_a_day(&self) -> bool {
        self.days_ahead.is_some()
    }

    /// Resolves "today"/"tomorrow" against the current tracking day.
    pub fn on(mut self, today: NaiveDate) -> Self {
        self.start_day = self
            .days_ahead
            .and_then(|days| today.checked_add_days(Days::new(days)));
        self
    }

    pub fn into_new_item(self) -> NewItem {
        NewItem {
            title: self.title,
            block: self.block,
            repeat: Some(self.repeat),
            weekdays: self.weekdays,
            planned_minutes: self.planned_minutes,
            date: self.start_day,
            ..NewItem::default()
        }
    }
}

fn prayer(word: &str) -> Option<BoundaryId> {
    match word {
        "fajr" | "fajir" | "subh" | "bomdod" => Some(BoundaryId::Fajr),
        "dhuhr" | "duhr" | "zuhr" | "zohr" | "dhuhur" | "peshin" => Some(BoundaryId::Dhuhr),
        "asr" | "asar" | "asir" => Some(BoundaryId::Asr),
        "maghrib" | "magrib" | "shom" => Some(BoundaryId::Maghrib),
        "isha" | "ishaa" | "esha" | "xufton" => Some(BoundaryId::Isha),
        _ => None,
    }
}

/// A block named after "after", "at", or "in": a prayer, or the night.
fn block_after_preposition(word: &str) -> Option<BoundaryId> {
    prayer(word).or(matches!(word, "night" | "tahajjud").then_some(BoundaryId::Night))
}

fn previous(block: BoundaryId) -> Option<BoundaryId> {
    BoundaryId::AWAKE
        .into_iter()
        .rev()
        .skip_while(|b| *b != block)
        .nth(1)
}

/// `20m`, `20min`, `2h`, `1h30m`, `1h30` → minutes.
fn duration(word: &str) -> Option<i64> {
    let digits = |s: &str| -> Option<i64> {
        (!s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
            .then(|| s.parse().ok())
            .flatten()
    };
    for unit in ["minutes", "minute", "mins", "min", "m"] {
        if let Some(n) = word.strip_suffix(unit).and_then(digits) {
            return Some(n);
        }
    }
    for unit in ["hours", "hour", "hrs", "hr", "h"] {
        if let Some(n) = word.strip_suffix(unit).and_then(digits) {
            return Some(n * 60);
        }
    }
    let (hours, rest) = word.split_once('h')?;
    let minutes = rest.strip_suffix('m').unwrap_or(rest);
    Some(digits(hours)? * 60 + digits(minutes)?)
}

/// "today" or "tomorrow" as days after the current tracking day.
fn days_ahead(word: &str) -> Option<u64> {
    match word {
        "today" => Some(0),
        "tomorrow" => Some(1),
        _ => None,
    }
}

fn is_number(word: &str) -> Option<i64> {
    (!word.is_empty() && word.chars().all(|c| c.is_ascii_digit()))
        .then(|| word.parse().ok())
        .flatten()
}

/// Weekday names in `words` starting at `start`, joined by commas or "and".
/// Returns the set and how many words it used.
fn weekday_run(words: &[String], start: usize) -> Option<(Weekdays, usize)> {
    let mut mask = 0u8;
    let mut used = 0;
    for word in &words[start..] {
        if word == "and" && mask != 0 {
            used += 1;
            continue;
        }
        let mut any = false;
        for part in word.split(',').filter(|p| !p.is_empty()) {
            match weekday_index(part) {
                Some(index) => {
                    mask |= 1 << index;
                    any = true;
                }
                None => return (mask != 0).then_some((Weekdays(mask), used)),
            }
        }
        if !any {
            break;
        }
        used += 1;
    }
    (mask != 0).then_some((Weekdays(mask), used))
}

pub fn parse(text: &str) -> Result<Draft, Error> {
    let original: Vec<&str> = text.split_whitespace().collect();
    // Matching ignores case and trailing punctuation; the title keeps the
    // user's own spelling.
    let words: Vec<String> = original
        .iter()
        .map(|w| {
            w.to_lowercase()
                .trim_end_matches(['.', ';', '!'])
                .trim_end_matches(',')
                .to_string()
        })
        .collect();
    let mut used = vec![false; words.len()];
    let mut draft = Draft {
        title: String::new(),
        block: None,
        repeat: Repeat::None,
        weekdays: None,
        planned_minutes: None,
        start_day: None,
        days_ahead: None,
    };
    let mut i = 0;
    while i < words.len() {
        let word = words[i].as_str();
        let next = words.get(i + 1).map(String::as_str);
        let mut take = 0;
        if matches!(word, "after" | "at" | "in") && draft.block.is_none() {
            if let Some(block) = next.and_then(block_after_preposition) {
                draft.block = Some(block);
                take = 2;
            }
        } else if word == "before" && draft.block.is_none() {
            if let Some(block) = next.and_then(prayer).and_then(previous) {
                draft.block = Some(block);
                take = 2;
            }
        } else if let Some(block) = prayer(word).filter(|_| draft.block.is_none()) {
            draft.block = Some(block);
            take = 1;
        } else if let Some(minutes) = duration(word).filter(|_| draft.planned_minutes.is_none()) {
            draft.planned_minutes = Some(minutes);
            take = 1;
        } else if let (Some(n), Some(unit)) = (is_number(word), next) {
            if draft.planned_minutes.is_none()
                && let Some(minutes) = duration(&format!("{n}{unit}"))
            {
                draft.planned_minutes = Some(minutes);
                take = 2;
            }
        } else if let Some(days) = days_ahead(word).filter(|_| draft.days_ahead.is_none()) {
            draft.days_ahead = Some(days);
            take = 1;
        } else if matches!(word, "daily" | "everyday") {
            draft.repeat = Repeat::Daily;
            take = 1;
        } else if word == "every" && next == Some("day") {
            draft.repeat = Repeat::Daily;
            take = 2;
        } else if word == "weekdays" || (word == "every" && next == Some("weekday")) {
            draft.repeat = Repeat::Weekly;
            draft.weekdays = Some(Weekdays::WORKWEEK);
            take = if word == "every" { 2 } else { 1 };
        } else if matches!(word, "every" | "on")
            && let Some((days, count)) = weekday_run(&words, i + 1)
        {
            draft.repeat = Repeat::Weekly;
            draft.weekdays = Some(days);
            take = 1 + count;
        }
        for flag in used.iter_mut().skip(i).take(take) {
            *flag = true;
        }
        i += take.max(1);
    }
    let title: Vec<&str> = original
        .iter()
        .zip(&used)
        .filter(|(_, used)| !**used)
        .map(|(word, _)| *word)
        .collect();
    let mut title = title.join(" ");
    if let Some(first) = title.chars().next() {
        title = first.to_uppercase().collect::<String>() + &title[first.len_utf8()..];
    }
    draft.title =
        validate_title(&title).map_err(|_| "add a title, e.g. \"read 20m after isha\"")?;
    Ok(draft)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_full_routine() {
        let draft = parse("read 20m after Isha daily").unwrap();
        assert_eq!(draft.title, "Read");
        assert_eq!(draft.block, Some(BoundaryId::Isha));
        assert_eq!(draft.repeat, Repeat::Daily);
        assert_eq!(draft.planned_minutes, Some(20));
    }

    #[test]
    fn understands_durations_and_weekdays() {
        let draft = parse("Deep work 1h30m before asr weekdays").unwrap();
        assert_eq!(draft.title, "Deep work");
        assert_eq!(draft.block, Some(BoundaryId::Dhuhr));
        assert_eq!(draft.weekdays, Some(Weekdays::WORKWEEK));
        assert_eq!(draft.planned_minutes, Some(90));

        let draft = parse("pushups every mon, wed and fri").unwrap();
        assert_eq!(draft.title, "Pushups");
        assert_eq!(draft.planned_minutes, None);
        assert_eq!(draft.weekdays.unwrap().names(), ["mon", "wed", "fri"]);

        let draft = parse("call mum 15 min on sundays").unwrap();
        assert_eq!(draft.title, "Call mum");
        assert_eq!(draft.planned_minutes, Some(15));
        assert_eq!(draft.weekdays.unwrap().names(), ["sun"]);
    }

    #[test]
    fn the_night_block_is_before_fajr_or_at_night() {
        assert_eq!(
            parse("tahajjud before fajr").unwrap().block,
            Some(BoundaryId::Night)
        );
        let draft = parse("study 45m at night").unwrap();
        assert_eq!(
            (draft.title.as_str(), draft.block),
            ("Study", Some(BoundaryId::Night))
        );
        assert_eq!(parse("night walk").unwrap().title, "Night walk");
    }

    #[test]
    fn plain_text_is_a_one_time_item_without_a_plan() {
        let draft = parse("Pay the electricity bill").unwrap();
        assert_eq!(draft.title, "Pay the electricity bill");
        assert_eq!(draft.block, None);
        assert_eq!(draft.repeat, Repeat::None);
        assert_eq!(draft.planned_minutes, None);
    }

    #[test]
    fn words_that_only_look_like_phrases_stay_in_the_title() {
        let draft = parse("meet at the office every so often").unwrap();
        assert_eq!(draft.title, "Meet at the office every so often");
        assert_eq!(draft.block, None);
        assert_eq!(draft.repeat, Repeat::None);
    }

    #[test]
    fn a_line_without_a_title_is_rejected() {
        assert!(parse("20m after isha daily").is_err());
        assert!(parse("   ").is_err());
        assert!(parse("tomorrow after fajr").is_err());
    }

    #[test]
    fn today_and_tomorrow_name_the_day_and_leave_the_title() {
        let today = NaiveDate::from_ymd_opt(2028, 2, 28).unwrap();
        let draft = parse("call mum tomorrow 15m after asr").unwrap();
        assert_eq!(draft.title, "Call mum");
        assert_eq!(draft.block, Some(BoundaryId::Asr));
        assert_eq!(draft.planned_minutes, Some(15));
        assert!(draft.names_a_day());
        assert_eq!(draft.start_day, None);
        let draft = draft.on(today);
        assert_eq!(draft.start_day, NaiveDate::from_ymd_opt(2028, 2, 29));
        assert_eq!(
            draft.into_new_item().date,
            NaiveDate::from_ymd_opt(2028, 2, 29)
        );

        let draft = parse("Today, read").unwrap().on(today);
        assert_eq!(
            (draft.title.as_str(), draft.start_day),
            ("Read", Some(today))
        );
        // Only the first named day counts; the other stays in the title.
        assert_eq!(parse("read today tomorrow").unwrap().title, "Read tomorrow");
        // A word that only starts with it is part of the title.
        let draft = parse("review today's notes").unwrap();
        assert_eq!(draft.title, "Review today's notes");
        assert!(!draft.names_a_day());
        assert_eq!(draft.on(today).start_day, None);
    }
}
