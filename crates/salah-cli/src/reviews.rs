//! Show a Monday-first week or save its reflection.
use crate::{
    local::Local,
    render,
    tracking::{Output, print},
};
use chrono::{Datelike, Duration, NaiveDate};
use clap::Subcommand;
use serde_json::json;

#[derive(Subcommand)]
pub enum ReviewCommand {
    /// Show the computed summary and saved reflection (default: last full week)
    Show {
        /// Any Monday; defaults to the Monday of the last full week
        #[arg(long, value_name = "YYYY-MM-DD")]
        week: Option<String>,
        #[command(flatten)]
        output: Output,
    },
    /// Save a reflection (an empty note clears it)
    Save {
        /// Any Monday; defaults to the Monday of the last full week
        #[arg(long, value_name = "YYYY-MM-DD")]
        week: Option<String>,
        /// The reflection text
        note: String,
        #[command(flatten)]
        output: Output,
    },
}

/// The Monday of the week before the one containing `today`.
pub fn last_full_week(today: NaiveDate) -> NaiveDate {
    today - Duration::days(i64::from(today.weekday().num_days_from_monday()) + 7)
}

fn week_or_default(local: &mut Local<'_>, week: Option<String>) -> Result<String, String> {
    if let Some(week) = week {
        return Ok(week);
    }
    let day = local.call("day.get", json!({}))?;
    let today = day["day"]
        .as_str()
        .and_then(|s| s.parse::<NaiveDate>().ok())
        .ok_or("day.get returned no day")?;
    Ok(last_full_week(today).to_string())
}

pub fn review(local: &mut Local<'_>, command: ReviewCommand) -> Result<(), String> {
    match command {
        ReviewCommand::Show { week, output } => {
            let week = week_or_default(local, week)?;
            let result = local.call("review.get", json!({"week":week}))?;
            print(&output, &result, render::review)
        }
        ReviewCommand::Save { week, note, output } => {
            let week = week_or_default(local, week)?;
            let result = local.mutate("review.save", json!({"week":week,"note":note}))?;
            print(&output, &result, render::mutation)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    #[test]
    fn the_default_week_is_the_last_full_monday_based_week() {
        assert_eq!(last_full_week(d("2031-05-14")), d("2031-05-05")); // Wed
        assert_eq!(last_full_week(d("2031-05-12")), d("2031-05-05")); // Mon
        assert_eq!(last_full_week(d("2031-05-18")), d("2031-05-05")); // Sun
        assert_eq!(last_full_week(d("2031-05-19")), d("2031-05-12"));
    }
}
