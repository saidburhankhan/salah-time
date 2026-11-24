//! Human goal commands use the same mutation and reporting paths as RPC.
use crate::{
    local::Local,
    render,
    tracking::{Output, Params, print},
};
use clap::{Args, Subcommand};
use serde_json::json;

#[derive(Subcommand)]
pub enum GoalCommand {
    /// List goals and their progress on a tracking day
    List {
        #[arg(long, value_name = "YYYY-MM-DD")]
        at: Option<String>,
        #[arg(long)]
        include_ended: bool,
        #[command(flatten)]
        output: Output,
    },
    /// Set a daily or weekly minutes or metric target
    Add(Add),
    /// Start a successor at the current period boundary
    Edit {
        #[command(flatten)]
        goal: GoalId,
        #[arg(long)]
        target: f64,
    },
    /// End an active goal
    End(GoalId),
    /// Remove a goal during its first period
    Remove(GoalId),
    /// Show progress and targets across a goal's edit history
    History {
        id: String,
        #[arg(long, value_name = "YYYY-MM-DD")]
        from: String,
        #[arg(long, value_name = "YYYY-MM-DD")]
        to: String,
        #[command(flatten)]
        output: Output,
    },
}

#[derive(Args)]
pub struct GoalId {
    id: String,
    #[arg(long)]
    expected_version: Option<i64>,
    #[command(flatten)]
    output: Output,
}

impl GoalId {
    fn params(&self) -> Params {
        Params::default()
            .set("id", self.id.as_str())
            .opt("expected_version", self.expected_version)
    }
}

#[derive(Args)]
pub struct Add {
    #[arg(long,value_parser=["day","week"])]
    period: String,
    #[arg(long)]
    target: f64,
    /// Omit for a minutes target
    #[arg(long, value_name = "ID")]
    metric: Option<String>,
    #[arg(long,value_parser=["at_least","at_most"])]
    direction: Option<String>,
    #[arg(long, group = "scope", value_name = "ID")]
    project: Option<String>,
    #[arg(long, group = "scope", value_name = "ID")]
    epic: Option<String>,
    #[arg(long, group = "scope", value_name = "ID")]
    item: Option<String>,
    #[command(flatten)]
    output: Output,
}

pub fn goal(local: &mut Local<'_>, command: GoalCommand) -> Result<(), String> {
    let (method, params, output) = match command {
        GoalCommand::List {
            at,
            include_ended,
            output,
        } => {
            let params = Params::default()
                .opt("at", at)
                .set("include_ended", include_ended);
            let result = local.call("goals.list", params.done())?;
            return print(&output, &result, render::goals);
        }
        GoalCommand::History {
            id,
            from,
            to,
            output,
        } => {
            let result = local.call("goals.history", json!({"id":id,"from":from,"to":to}))?;
            return print(&output, &result, render::goal_history);
        }
        GoalCommand::Add(a) => {
            let item = a.item.as_deref().map(|id| local.item_id(id)).transpose()?;
            let params = Params::default()
                .set("period", a.period)
                .set("target", a.target)
                .opt("project_id", a.project)
                .opt("epic_id", a.epic)
                .opt("item_id", item)
                .opt("metric_id", a.metric)
                .opt("direction", a.direction);
            ("goal.add", params, a.output)
        }
        GoalCommand::Edit { goal, target } => (
            "goal.edit",
            goal.params().set("target", target),
            goal.output,
        ),
        GoalCommand::End(goal) => ("goal.end", goal.params(), goal.output),
        GoalCommand::Remove(goal) => ("goal.remove", goal.params(), goal.output),
    };
    let result = local.mutate(method, params.done())?;
    print(&output, &result, render::mutation)
}
