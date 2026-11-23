//! Resources: notes, links and reminders on exactly one item or epic. A
//! resource's kind never changes; each kind needs its own field (a note its
//! `body`, a link an http(s) `url`, a reminder its `remind_at`).

use crate::guard::{ensure_version, label, not_found, nullable, stamp};
use crate::{Store, WriteTx, epics, items};
use chrono::{DateTime, Utc};
use salah_domain::{
    error::Error,
    resource::{Resource, ResourceKind},
    runtime::IdGenerator,
};
use salah_store::resources::Parent;
use serde::Deserialize;

const TITLE_MAX: usize = 200;
const BODY_MAX: usize = 5000;
const URL_MAX: usize = 2000;

/// The item or the epic a request names; exactly one of them.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Owner {
    #[serde(default)]
    pub item_id: Option<String>,
    #[serde(default)]
    pub epic_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct NewResource {
    #[serde(flatten)]
    pub owner: Owner,
    pub kind: ResourceKind,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub remind_at: Option<DateTime<Utc>>,
}

/// Absent fields keep their value; `null` clears an optional one.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ResourceChanges {
    #[serde(default, deserialize_with = "nullable")]
    pub title: Option<Option<String>>,
    #[serde(default, deserialize_with = "nullable")]
    pub body: Option<Option<String>>,
    #[serde(default, deserialize_with = "nullable")]
    pub url: Option<Option<String>>,
    #[serde(default, deserialize_with = "nullable")]
    pub remind_at: Option<Option<DateTime<Utc>>>,
    #[serde(default)]
    pub done: Option<bool>,
}

impl Owner {
    /// Checks that exactly one live parent is named.
    fn parent(&self, store: &Store) -> Result<Parent<'_>, Error> {
        match (&self.item_id, &self.epic_id) {
            (Some(id), None) => {
                items::find(store, id)?;
                Ok(Parent::Item(id))
            }
            (None, Some(id)) => {
                epics::find(store, id)?;
                Ok(Parent::Epic(id))
            }
            _ => Err(Error::validation(
                "item_id",
                "name exactly one of item_id or epic_id",
            )),
        }
    }
}

pub fn list(store: &Store, owner: &Owner) -> Result<Vec<Resource>, Error> {
    store.resources_of(owner.parent(store)?)
}

pub fn find(store: &Store, id: &str) -> Result<Resource, Error> {
    store
        .resource(id)?
        .filter(|r| r.deleted_at.is_none())
        .ok_or_else(|| not_found("resource", id))
}

pub fn add(
    tx: &WriteTx<'_>,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    new: NewResource,
) -> Result<Resource, Error> {
    let parent = new.owner.parent(tx)?;
    let mut resource = Resource {
        id: ids.new_id(),
        item_id: new.owner.item_id.clone(),
        epic_id: new.owner.epic_id.clone(),
        kind: new.kind,
        title: text("title", new.title, TITLE_MAX)?,
        body: text("body", new.body, BODY_MAX)?,
        url: url(new.url)?,
        remind_at: new.remind_at.map(stamp),
        done: false,
        position: tx.next_resource_position(parent)?,
        created_at: now,
        updated_at: now,
        deleted_at: None,
        version: 1,
    };
    check_kind(&resource)?;
    resource.version = tx.put_resource(&resource)?;
    Ok(resource)
}

pub fn edit(
    tx: &WriteTx<'_>,
    now: DateTime<Utc>,
    id: &str,
    changes: ResourceChanges,
    expected_version: Option<i64>,
) -> Result<Resource, Error> {
    let mut resource = find(tx, id)?;
    ensure_version("resource", id, resource.version, expected_version)?;
    if let Some(title) = changes.title {
        resource.title = text("title", title, TITLE_MAX)?;
    }
    if let Some(body) = changes.body {
        resource.body = text("body", body, BODY_MAX)?;
    }
    if let Some(value) = changes.url {
        resource.url = url(value)?;
    }
    if let Some(remind_at) = changes.remind_at {
        resource.remind_at = remind_at.map(stamp);
    }
    if let Some(done) = changes.done {
        if resource.kind != ResourceKind::Reminder {
            return Err(Error::validation("done", "only a reminder can be done"));
        }
        resource.done = done;
    }
    check_kind(&resource)?;
    resource.updated_at = now;
    resource.version = tx.put_resource(&resource)?;
    Ok(resource)
}

pub fn remove(
    tx: &WriteTx<'_>,
    now: DateTime<Utc>,
    id: &str,
    expected_version: Option<i64>,
) -> Result<Resource, Error> {
    let mut resource = find(tx, id)?;
    ensure_version("resource", id, resource.version, expected_version)?;
    resource.deleted_at = Some(now);
    resource.updated_at = now;
    resource.version = tx.put_resource(&resource)?;
    Ok(resource)
}

/// Each kind needs its own field.
fn check_kind(r: &Resource) -> Result<(), Error> {
    let (needs, field) = match r.kind {
        ResourceKind::Note => (r.body.is_some(), "body"),
        ResourceKind::Link => (r.url.is_some(), "url"),
        ResourceKind::Reminder => (r.remind_at.is_some(), "remind_at"),
    };
    if !needs {
        return Err(Error::validation(
            field,
            format!("a {} needs {field}", kind_name(r.kind)),
        ));
    }
    Ok(())
}

fn kind_name(kind: ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Note => "note",
        ResourceKind::Link => "link",
        ResourceKind::Reminder => "reminder",
    }
}

/// Optional text: trimmed, empty means none. Only a body keeps line breaks.
fn text(field: &'static str, value: Option<String>, max: usize) -> Result<Option<String>, Error> {
    let Some(value) = value.filter(|v| !v.trim().is_empty()) else {
        return Ok(None);
    };
    if field != "body" {
        return label(field, &value, max).map(Some);
    }
    let value = value.trim();
    if value.chars().count() > max {
        return Err(Error::validation(
            field,
            format!("{field} must be at most {max} characters"),
        ));
    }
    if value
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(Error::validation(
            field,
            format!("{field} must not contain control characters"),
        ));
    }
    Ok(Some(value.to_owned()))
}

/// An absolute http or https URL with a host and no spaces.
fn url(value: Option<String>) -> Result<Option<String>, Error> {
    let Some(value) = text("url", value, URL_MAX)? else {
        return Ok(None);
    };
    let lower = value.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"));
    let authority = rest.and_then(|rest| rest.split(['/', '?', '#']).next());
    if value.contains('\\')
        || value.chars().any(char::is_whitespace)
        || !authority.is_some_and(valid_authority)
    {
        return Err(Error::validation(
            "url",
            "a link needs an http:// or https:// url",
        ));
    }
    Ok(Some(value))
}

fn valid_authority(authority: &str) -> bool {
    let host_port = authority.rsplit('@').next().unwrap_or_default();
    let valid_port = |port: &str| {
        port.bytes().all(|digit| digit.is_ascii_digit())
            && (port.is_empty() || port.parse::<u16>().is_ok())
    };
    if let Some(ip) = host_port.strip_prefix('[') {
        return ip.split_once(']').is_some_and(|(ip, suffix)| {
            ip.parse::<std::net::Ipv6Addr>().is_ok()
                && (suffix.is_empty() || suffix.strip_prefix(':').is_some_and(valid_port))
        });
    }
    let (host, port) = host_port.split_once(':').unwrap_or((host_port, ""));
    !host.is_empty()
        && host
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | '_'))
        && valid_port(port)
}

#[cfg(test)]
mod tests {
    use super::url;

    #[test]
    fn links_need_an_http_or_https_url_with_a_host() {
        for good in [
            "https://example.com",
            "HTTP://a.b/c?d#e",
            "http://[::1]:8080/",
            "https://bücher.example/",
        ] {
            assert_eq!(url(Some(good.into())).unwrap().as_deref(), Some(good));
        }
        for bad in [
            "ftp://x",
            "example.com",
            "https://",
            "https:///path",
            "javascript:alert(1)",
            "https://x.org/a b",
            "https://:443/path",
            "http://example.com:65536",
            "https://user@",
            "https://[invalid]",
            "https://\\example.org",
        ] {
            assert!(url(Some(bad.into())).is_err(), "{bad}");
        }
        assert_eq!(url(Some("  ".into())).unwrap(), None);
    }
}
