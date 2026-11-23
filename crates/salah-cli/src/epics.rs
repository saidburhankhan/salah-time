//! `salah epic`: optional sub-goals inside a project. Each subcommand calls
//! one epic method; `finish` and `reopen` are `epic.edit` with a status.

use crate::local::Local;
use crate::render;
use crate::tracking::{Output, Params, print};
use clap::{Args, Subcommand};
use serde_json::{Value, json};

#[derive(Subcommand)]
pub enum EpicCommand {
    /// List epics with their recorded work
    List(EpicListArgs),
    /// Add an epic to a project
    Add(EpicAddArgs),
    /// Rename an epic or change its note
    Edit(EpicEditArgs),
    /// Mark an epic finished
    Finish(EpicIdArgs),
    /// Make a finished epic active again
    Reopen(EpicIdArgs),
    /// Remove an epic without sessions (finish one that has work)
    Remove(EpicIdArgs),
}

#[derive(Args)]
pub struct EpicListArgs {
    #[arg(long, value_name = "ID")]
    project: Option<String>,
    /// active or finished
    #[arg(long)]
    status: Option<String>,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
pub struct EpicAddArgs {
    name: String,
    #[arg(long, value_name = "ID")]
    project: String,
    #[arg(long)]
    note: Option<String>,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
pub struct EpicEditArgs {
    id: String,
    #[arg(long)]
    name: Option<String>,
    #[arg(long, conflicts_with = "no_note")]
    note: Option<String>,
    /// Clear the note
    #[arg(long)]
    no_note: bool,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
pub struct EpicIdArgs {
    id: String,
    #[command(flatten)]
    output: Output,
}

pub fn epic(local: &mut Local<'_>, command: EpicCommand) -> Result<(), String> {
    let (method, params, output) = match command {
        EpicCommand::List(a) => {
            let params = Params::default()
                .opt("project_id", a.project)
                .opt("status", a.status);
            let result = local.call("epics.list", params.done())?;
            return print(&a.output, &result, render::epics);
        }
        EpicCommand::Add(a) => {
            let params = Params::default()
                .set("project_id", a.project)
                .set("name", a.name)
                .opt("note", a.note);
            ("epic.add", params.done(), a.output)
        }
        EpicCommand::Edit(a) => {
            let mut params = Params::default()
                .set("id", a.id)
                .opt("name", a.name)
                .opt("note", a.note);
            if a.no_note {
                params = params.set("note", Value::Null);
            }
            ("epic.edit", params.done(), a.output)
        }
        EpicCommand::Finish(a) => (
            "epic.edit",
            json!({"id": a.id, "status": "finished"}),
            a.output,
        ),
        EpicCommand::Reopen(a) => (
            "epic.edit",
            json!({"id": a.id, "status": "active"}),
            a.output,
        ),
        EpicCommand::Remove(a) => ("epic.remove", json!({"id": a.id}), a.output),
    };
    let result = local.mutate(method, params)?;
    print(&output, &result, render::mutation)
}
