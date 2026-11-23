//! Projects: what time is spent on, with the tags they carry and the metrics
//! a finished session of theirs asks for.

use crate::guard::{color, ensure_version, label, not_found, note, nullable};
use crate::{Store, WriteTx, metrics, tags};
use chrono::{DateTime, Utc};
use salah_domain::{
    error::Error,
    runtime::IdGenerator,
    tracking::{Metric, Project, ProjectMetric, Tag},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const NAME_MAX: usize = 60;
const DEFAULT_COLOR: &str = "#4c8bf5";
/// Metrics a new project asks for unless the caller chooses its own.
const DEFAULT_METRICS: [&str; 2] = ["focus", "quality"];

/// One metric a project asks for, in list order.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct MetricChoice {
    pub metric_id: String,
    pub required: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct NewProject {
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub tag_ids: Vec<String>,
    /// Defaults to the built-in `focus` and `quality`, with the legacy `required` flags set.
    #[serde(default)]
    pub metrics: Option<Vec<MetricChoice>>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ProjectChanges {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    /// `null` clears the note; an absent field keeps it.
    #[serde(default, deserialize_with = "nullable")]
    pub note: Option<Option<String>>,
    #[serde(default)]
    pub tag_ids: Option<Vec<String>>,
    #[serde(default)]
    pub metrics: Option<Vec<MetricChoice>>,
    #[serde(default)]
    pub archived: Option<bool>,
}

/// A project with its tags and the metrics it asks for.
#[derive(Clone, Debug, Serialize)]
pub struct ProjectView {
    #[serde(flatten)]
    pub project: Project,
    pub tags: Vec<Tag>,
    pub metrics: Vec<ProjectMetricView>,
}

/// A project's metric link joined with the metric itself.
#[derive(Clone, Debug, Serialize)]
pub struct ProjectMetricView {
    #[serde(flatten)]
    pub link: ProjectMetric,
    pub metric: Metric,
}

/// Live projects in creation order; `archived` filters by archive state.
pub fn list(store: &Store, archived: Option<bool>) -> Result<Vec<ProjectView>, Error> {
    store
        .projects()?
        .into_iter()
        .filter(|p| p.deleted_at.is_none() && archived.is_none_or(|a| p.archived == a))
        .map(|p| view(store, p))
        .collect()
}

pub fn view(store: &Store, project: Project) -> Result<ProjectView, Error> {
    let tags = project
        .tag_ids
        .iter()
        .map(|id| tags::find(store, id))
        .collect::<Result<_, _>>()?;
    let metrics = store
        .project_metrics(&project.id)?
        .into_iter()
        .map(|link| {
            Ok(ProjectMetricView {
                metric: metrics::find(store, &link.metric_id)?,
                link,
            })
        })
        .collect::<Result<_, Error>>()?;
    Ok(ProjectView {
        project,
        tags,
        metrics,
    })
}

/// A live project, archived or not.
pub fn find(store: &Store, id: &str) -> Result<Project, Error> {
    store
        .project(id)?
        .filter(|p| p.deleted_at.is_none())
        .ok_or_else(|| not_found("project", id))
}

/// A live project that is not archived, as new work requires.
pub fn active(store: &Store, id: &str) -> Result<Project, Error> {
    let project = find(store, id)?;
    if project.archived {
        return Err(Error::validation(
            "project_id",
            format!("project {:?} is archived", project.name),
        ));
    }
    Ok(project)
}

pub fn add(
    tx: &WriteTx<'_>,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    new: NewProject,
) -> Result<Project, Error> {
    let mut project = Project {
        id: ids.new_id(),
        name: label("name", &new.name, NAME_MAX)?,
        color: color(new.color.as_deref().unwrap_or(DEFAULT_COLOR))?,
        note: note(new.note)?,
        archived: false,
        tag_ids: checked_tags(tx, new.tag_ids)?,
        created_at: now,
        updated_at: now,
        deleted_at: None,
        version: 1,
    };
    let choices = new.metrics.unwrap_or_else(|| {
        DEFAULT_METRICS
            .map(|id| MetricChoice {
                metric_id: id.into(),
                required: true,
            })
            .into()
    });
    let links = checked_metrics(tx, &project.id, choices)?;
    project.version = tx.put_project(&project)?;
    tx.set_project_metrics(&project.id, &links)?;
    Ok(project)
}

pub fn edit(
    tx: &WriteTx<'_>,
    now: DateTime<Utc>,
    id: &str,
    changes: ProjectChanges,
    expected_version: Option<i64>,
) -> Result<Project, Error> {
    let mut project = find(tx, id)?;
    ensure_version("project", id, project.version, expected_version)?;
    if let Some(name) = &changes.name {
        project.name = label("name", name, NAME_MAX)?;
    }
    if let Some(value) = &changes.color {
        project.color = color(value)?;
    }
    if let Some(value) = changes.note {
        project.note = note(value)?;
    }
    if let Some(tag_ids) = changes.tag_ids {
        project.tag_ids = checked_tags(tx, tag_ids)?;
    }
    if let Some(archived) = changes.archived {
        project.archived = archived;
    }
    let links = changes
        .metrics
        .map(|choices| checked_metrics(tx, &project.id, choices))
        .transpose()?;
    project.updated_at = now;
    project.version = tx.put_project(&project)?;
    if let Some(links) = links {
        tx.set_project_metrics(&project.id, &links)?;
    }
    Ok(project)
}

fn checked_tags(store: &Store, tag_ids: Vec<String>) -> Result<Vec<String>, Error> {
    let mut seen = BTreeSet::new();
    for id in &tag_ids {
        tags::find(store, id)?;
        if !seen.insert(id) {
            return Err(Error::validation(
                "tag_ids",
                format!("tag {id:?} is listed twice"),
            ));
        }
    }
    Ok(tag_ids)
}

fn checked_metrics(
    store: &Store,
    project_id: &str,
    choices: Vec<MetricChoice>,
) -> Result<Vec<ProjectMetric>, Error> {
    let mut seen = BTreeSet::new();
    choices
        .into_iter()
        .enumerate()
        .map(|(position, choice)| {
            let metric = metrics::find(store, &choice.metric_id)?;
            if metric.archived {
                return Err(Error::validation(
                    "metrics",
                    format!("metric {:?} is archived", metric.name),
                ));
            }
            if !seen.insert(metric.id.clone()) {
                return Err(Error::validation(
                    "metrics",
                    format!("metric {:?} is listed twice", metric.name),
                ));
            }
            Ok(ProjectMetric {
                project_id: project_id.to_owned(),
                metric_id: metric.id,
                required: choice.required,
                position: position as i64,
            })
        })
        .collect()
}
