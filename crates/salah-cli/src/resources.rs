use crate::local::Local;
use crate::render;
use crate::tracking::{Output, Params, print};
use clap::{Args, Subcommand};

#[derive(Subcommand)]
pub enum ResourceCommand {
    /// List resources on one task or epic
    List(ResourceListArgs),
    /// Add a note, link or reminder
    Add(ResourceAddArgs),
    /// Edit a resource or tick off a reminder
    Edit(ResourceEditArgs),
    /// Remove a resource
    Remove(ResourceRemoveArgs),
}

#[derive(Args)]
#[group(required = true, multiple = false)]
struct ParentArgs {
    /// Item id or unique prefix
    #[arg(long, value_name = "ID")]
    item: Option<String>,
    /// Epic id
    #[arg(long, value_name = "ID")]
    epic: Option<String>,
}

impl ParentArgs {
    fn params(self, local: &Local<'_>) -> Result<Params, String> {
        let item = self.item.map(|id| local.item_id(&id)).transpose()?;
        Ok(Params::default()
            .opt("item_id", item)
            .opt("epic_id", self.epic))
    }
}

#[derive(Args)]
struct ContentArgs {
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    body: Option<String>,
    #[arg(long)]
    url: Option<String>,
    #[arg(long, value_name = "RFC3339")]
    remind_at: Option<String>,
}

impl ContentArgs {
    fn apply(self, params: Params) -> Params {
        params
            .opt("title", self.title)
            .opt("body", self.body)
            .opt("url", self.url)
            .opt("remind_at", self.remind_at)
    }
}

#[derive(Args)]
pub struct ResourceListArgs {
    #[command(flatten)]
    parent: ParentArgs,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
pub struct ResourceAddArgs {
    #[command(flatten)]
    parent: ParentArgs,
    #[arg(long, value_parser = ["note", "link", "reminder"])]
    kind: String,
    #[command(flatten)]
    content: ContentArgs,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
pub struct ResourceEditArgs {
    id: String,
    #[command(flatten)]
    content: ContentArgs,
    /// Mark a reminder done
    #[arg(long, conflicts_with = "undone")]
    done: bool,
    /// Reopen a reminder
    #[arg(long)]
    undone: bool,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
pub struct ResourceRemoveArgs {
    id: String,
    #[command(flatten)]
    output: Output,
}

pub fn resource(local: &mut Local<'_>, command: ResourceCommand) -> Result<(), String> {
    let (method, params, output) = match command {
        ResourceCommand::List(args) => {
            let result = local.call("resources.list", args.parent.params(local)?.done())?;
            return print(&args.output, &result, render::resources);
        }
        ResourceCommand::Add(args) => (
            "resource.add",
            args.content
                .apply(args.parent.params(local)?.set("kind", args.kind)),
            args.output,
        ),
        ResourceCommand::Edit(args) => (
            "resource.edit",
            args.content
                .apply(Params::default().set("id", args.id))
                .opt("done", (args.done || args.undone).then_some(args.done)),
            args.output,
        ),
        ResourceCommand::Remove(args) => (
            "resource.remove",
            Params::default().set("id", args.id),
            args.output,
        ),
    };
    let result = local.mutate(method, params.done())?;
    print(&output, &result, render::mutation)
}
