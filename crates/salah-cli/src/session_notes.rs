//! CLI writes for timestamped session notes.

use crate::{
    local::Local,
    render,
    tracking::{Output, print},
};
use clap::Args;
use serde_json::json;

#[derive(Args)]
pub struct Add {
    text: String,
    /// Session id (defaults to the running timer)
    #[arg(long)]
    id: Option<String>,
    /// Local HH:MM within the session, required for finished work
    #[arg(long)]
    at: Option<String>,
    #[arg(long)]
    expected_version: Option<i64>,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
pub struct Target {
    session_id: String,
    note_id: String,
    /// Require a specific session version (defaults to the current version)
    #[arg(long)]
    expected_version: Option<i64>,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
pub struct Edit {
    #[command(flatten)]
    target: Target,
    text: String,
}

pub fn add(local: &mut Local<'_>, args: Add) -> Result<(), String> {
    let result=local.mutate("session.note", json!({"id":args.id,"text":args.text,"at":args.at,"expected_version":args.expected_version}))?;
    print(&args.output, &result, render::mutation)
}

pub fn edit(local: &mut Local<'_>, args: Edit) -> Result<(), String> {
    change(local, args.target, Some(args.text))
}

pub fn remove(local: &mut Local<'_>, target: Target) -> Result<(), String> {
    change(local, target, None)
}

fn change(local: &mut Local<'_>, target: Target, text: Option<String>) -> Result<(), String> {
    let version = match target.expected_version {
        Some(version) => version,
        None => salah_core::sessions::find(&local.store, &target.session_id)?.version,
    };
    let mut params =
        json!({"session_id":target.session_id,"note_id":target.note_id,"expected_version":version});
    let method = if let Some(text) = text {
        params["text"] = json!(text);
        "session.note_edit"
    } else {
        "session.note_remove"
    };
    let result = local.mutate(method, params)?;
    print(&target.output, &result, render::mutation)
}
