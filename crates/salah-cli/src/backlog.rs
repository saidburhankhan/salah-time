//! `salah backlog`: one-time tasks without a day or block, and scheduling
//! them into a day's block.

use crate::local::Local;
use crate::render;
use crate::tracking::{Output, Params, print};
use clap::{Args, Subcommand};
use serde_json::Value;

#[derive(Subcommand)]
pub enum BacklogCommand {
    /// List the backlog in its order
    List(BacklogListArgs),
    /// Add a one-time task to the backlog
    Add(BacklogAddArgs),
    /// Give a task a day and a block
    Schedule(BacklogScheduleArgs),
}

#[derive(Args)]
pub struct BacklogListArgs {
    /// Project id, or `none` for tasks without a project
    #[arg(long, value_name = "ID")]
    project: Option<String>,
    /// Tag id, or `none` for tasks without a tag
    #[arg(long, value_name = "ID")]
    tag: Option<String>,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
pub struct BacklogAddArgs {
    /// Title, or with --quick a line such as "read 20m"
    #[arg(required = true, num_args = 1..)]
    title: Vec<String>,
    /// Parse the title as a quick-add line (its day and block are ignored)
    #[arg(long)]
    quick: bool,
    #[arg(long, value_name = "ID")]
    project: Option<String>,
    /// Without --project the task joins the epic's project
    #[arg(long, value_name = "ID")]
    epic: Option<String>,
    #[arg(long, value_name = "MINUTES")]
    planned_minutes: Option<i64>,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
pub struct BacklogScheduleArgs {
    /// Item id or unique prefix
    id: String,
    #[arg(long, value_name = "YYYY-MM-DD")]
    date: String,
    #[arg(long)]
    block: String,
    #[command(flatten)]
    output: Output,
}

pub fn backlog(local: &mut Local<'_>, command: BacklogCommand) -> Result<(), String> {
    match command {
        BacklogCommand::List(a) => {
            let params = Params::default()
                .opt("project_id", a.project)
                .opt("tag_id", a.tag);
            let result = local.call("backlog.list", params.done())?;
            print(&a.output, &result, render::backlog)
        }
        BacklogCommand::Add(a) => {
            let params = Params::default()
                .set(
                    if a.quick { "quick_text" } else { "title" },
                    a.title.join(" "),
                )
                .set("backlog", true)
                .opt("project_id", a.project)
                .opt("epic_id", a.epic)
                .opt("planned_minutes", a.planned_minutes);
            let result = local.mutate("item.add", params.done())?;
            print(&a.output, &result, render::mutation)
        }
        BacklogCommand::Schedule(a) => {
            let params = Params::default()
                .set("id", local.item_id(&a.id)?)
                .set("day", a.date)
                .set("block", Value::from(a.block));
            let result = local.mutate("item.schedule", params.done())?;
            print(&a.output, &result, render::mutation)
        }
    }
}
