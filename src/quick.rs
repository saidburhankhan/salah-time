//! Quick add: turns one line such as "read 20m after isha daily" into an
//! item draft. Words that are not part of a recognised phrase form the title.

use crate::day::BoundaryId;
use crate::item::{Measure, Repeat, Weekdays, validate_title, weekday_index};
use crate::planner::NewItem;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Draft {
    pub title: String,
    pub block: Option<BoundaryId>,
    pub repeat: Repeat,
    pub weekdays: Option<Weekdays>,
    pub measure: Measure,
    pub target: Option<i64>,
}

impl Draft {
    pub fn into_new_item(self) -> NewItem {
        NewItem {
            title: self.title,
            block: self.block,
            repeat: Some(self.repeat),
            weekdays: self.weekdays,
            measure: Some(self.measure),
            target: self.target,
            date: None,
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

/// `x3`, `3x` → count.
fn times(word: &str) -> Option<i64> {
    let n = word.strip_prefix('x').or_else(|| word.strip_suffix('x'))?;
    (!n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
        .then(|| n.parse().ok())
        .flatten()
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

pub fn parse(text: &str) -> Result<Draft, String> {
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
        measure: Measure::Check,
        target: None,
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
        } else if let Some(minutes) = duration(word).filter(|_| draft.target.is_none()) {
            draft.measure = Measure::Minutes;
            draft.target = Some(minutes);
            take = 1;
        } else if let Some(count) = times(word).filter(|_| draft.target.is_none()) {
            draft.measure = Measure::Count;
            draft.target = Some(count);
            take = 1;
        } else if let (Some(n), Some(unit)) = (is_number(word), next) {
            if draft.target.is_none() {
                if let Some(minutes) = duration(&format!("{n}{unit}")) {
                    draft.measure = Measure::Minutes;
                    draft.target = Some(minutes);
                    take = 2;
                } else if matches!(unit, "times" | "time" | "x") {
                    draft.measure = Measure::Count;
                    draft.target = Some(n);
                    take = 2;
                }
            }
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
        assert_eq!((draft.measure, draft.target), (Measure::Minutes, Some(20)));
    }

    #[test]
    fn understands_durations_counts_and_weekdays() {
        let draft = parse("Deep work 1h30m before asr weekdays").unwrap();
        assert_eq!(draft.title, "Deep work");
        assert_eq!(draft.block, Some(BoundaryId::Dhuhr));
        assert_eq!(draft.weekdays, Some(Weekdays::WORKWEEK));
        assert_eq!(draft.target, Some(90));

        let draft = parse("pushups 30 times every mon, wed and fri").unwrap();
        assert_eq!(draft.title, "Pushups");
        assert_eq!((draft.measure, draft.target), (Measure::Count, Some(30)));
        assert_eq!(draft.weekdays.unwrap().names(), ["mon", "wed", "fri"]);

        let draft = parse("call mum 15 min on sundays").unwrap();
        assert_eq!(draft.title, "Call mum");
        assert_eq!(draft.target, Some(15));
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
    fn plain_text_is_a_one_time_tick() {
        let draft = parse("Pay the electricity bill").unwrap();
        assert_eq!(draft.title, "Pay the electricity bill");
        assert_eq!(draft.block, None);
        assert_eq!(draft.repeat, Repeat::None);
        assert_eq!(draft.measure, Measure::Check);
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
    }
}
