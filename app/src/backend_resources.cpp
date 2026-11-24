// Backend reads and writes for resources (notes, links, reminders) attached to
// an item or an epic, kept apart from backend.cpp. The list is daemon-only
// (a read); writes use the shared socket-or-stdio path, and a successful write
// is merged into the list so it stays right when the daemon is offline.
#include "backend.h"

#include <QJsonArray>
#include <QJsonObject>

namespace {
const QStringList kTextFields{QStringLiteral("title"), QStringLiteral("body"), QStringLiteral("url"),
                              QStringLiteral("remind_at")};
}

void Backend::loadResources(const QString &kind, const QString &id) {
    const bool same = m_resourceKind == kind && m_resourceTarget == id;
    m_resourceKind = kind == QLatin1String("epic") ? kind : QStringLiteral("item");
    m_resourceTarget = id;
    m_resourcesOpen = !id.isEmpty();
    if (!same) m_resources.clear();
    refreshResources();
}

void Backend::closeResources() {
    m_resourcesOpen = false;
    ++m_resourcesRequest;
    m_resourceTarget.clear();
    m_resources.clear();
    m_resourcesLoading = false;
    emit resourcesChanged();
}

void Backend::refreshResources() {
    if (!m_resourcesOpen || m_resourceTarget.isEmpty()) return;
    if (m_client.offline()) {
        m_resourcesLoading = false;
        emit resourcesChanged();
        return;
    }
    const quint64 request = ++m_resourcesRequest;
    m_resourcesLoading = true;
    emit resourcesChanged();
    const QString key = m_resourceKind == QLatin1String("epic") ? QStringLiteral("epic_id") : QStringLiteral("item_id");
    m_client.rpcRead(QStringLiteral("resources.list"), QJsonObject{{key, m_resourceTarget}},
                     [this, request](const CoreClient::Result &result) {
        if (request != m_resourcesRequest) return;
        m_resourcesLoading = false;
        if (const auto *data = std::get_if<Dto::Json>(&result.value)) {
            m_resources = data->value.toArray().toVariantList();
        } else if (!result.ok && !m_client.offline()) {
            fail(result.error.message);
        }
        emit resourcesChanged();
    });
}

qint64 Backend::resourceVersion(const QString &id) const {
    for (const QVariant &entry : m_resources) {
        const QVariantMap row = entry.toMap();
        if (row.value(QStringLiteral("id")).toString() == id) return row.value(QStringLiteral("version"), -1).toLongLong();
    }
    return -1;
}

void Backend::addResource(const QVariantMap &fields) {
    if (m_resourceTarget.isEmpty()) return;
    const QString key = m_resourceKind == QLatin1String("epic") ? QStringLiteral("epic_id") : QStringLiteral("item_id");
    QJsonObject params{{key, m_resourceTarget}, {QStringLiteral("kind"), fields.value(QStringLiteral("kind")).toString()}};
    for (const QString &name : kTextFields) {
        const QString text = fields.value(name).toString();
        if (!text.trimmed().isEmpty()) params.insert(name, text);
    }
    m_client.mutateEntity(QStringLiteral("resource.add"), params,
                          [this](const CoreClient::Result &result) { applyResourceWrite(result); });
}

void Backend::attachTaskResource(const QString &id, const QVariantMap &fields, const QString &operation) {
    QJsonObject params = QJsonObject::fromVariantMap(fields);
    params.insert("item_id", id);
    m_client.mutateEntity("resource.add", params, [this, operation](const CoreClient::Result &result) {
        if (result.ok) m_refreshPending = true;
        emit attachmentFinished(operation, result.ok ? QString() : result.error.message,
                                !result.ok && result.error.definitiveRejection());
    }, operation);
}

void Backend::editResource(const QString &id, const QVariantMap &fields) {
    if (id.isEmpty()) return;
    QJsonObject params{{QStringLiteral("id"), id}};
    const qint64 version = resourceVersion(id);
    if (version >= 0) params.insert(QStringLiteral("expected_version"), version);
    for (const QString &name : kTextFields) {
        if (!fields.contains(name)) continue;
        const QString text = fields.value(name).toString();
        params.insert(name, text.trimmed().isEmpty() ? QJsonValue(QJsonValue::Null) : QJsonValue(text));
    }
    if (fields.contains(QStringLiteral("done"))) params.insert(QStringLiteral("done"), fields.value(QStringLiteral("done")).toBool());
    m_client.mutateEntity(QStringLiteral("resource.edit"), params,
                          [this](const CoreClient::Result &result) { applyResourceWrite(result); });
}

void Backend::removeResource(const QString &id) {
    if (id.isEmpty()) return;
    QJsonObject params{{QStringLiteral("id"), id}};
    const qint64 version = resourceVersion(id);
    if (version >= 0) params.insert(QStringLiteral("expected_version"), version);
    m_client.mutateEntity(QStringLiteral("resource.remove"), params,
                          [this](const CoreClient::Result &result) { applyResourceWrite(result); });
}

void Backend::applyResourceWrite(const CoreClient::Result &result) {
    if (!result.ok) {
        fail(result.error.message);
        emit resourceWriteFinished(false, result.error.message);
        // A conflict means the list is stale.
        if (result.error.kind == CoreClient::ErrorKind::Conflict) refreshResources();
        return;
    }
    if (const auto *mutation = std::get_if<Dto::MutationResult>(&result.value)) {
        const QJsonObject changed = mutation->json.value(QStringLiteral("changed")).toObject();
        for (const QJsonValue &value : changed.value(QStringLiteral("deleted")).toArray()) {
            for (int i = 0; i < m_resources.size(); ++i)
                if (m_resources.at(i).toMap().value(QStringLiteral("id")).toString() == value.toString()) {
                    m_resources.removeAt(i);
                    break;
                }
        }
        for (const QJsonValue &value : changed.value(QStringLiteral("resources")).toArray()) {
            const QVariantMap row = value.toObject().toVariantMap();
            const QString id = row.value(QStringLiteral("id")).toString();
            const QString owner = m_resourceKind == QLatin1String("epic") ? row.value(QStringLiteral("epic_id")).toString()
                                                                          : row.value(QStringLiteral("item_id")).toString();
            if (owner != m_resourceTarget) continue;
            bool replaced = false;
            for (int i = 0; i < m_resources.size() && !replaced; ++i)
                if (m_resources.at(i).toMap().value(QStringLiteral("id")).toString() == id) {
                    m_resources[i] = row;
                    replaced = true;
                }
            if (!replaced) m_resources.append(row);
        }
        emit resourcesChanged();
    }
    emit resourceWriteFinished(true, QString());
    // Day and backlog rows carry resource counts.
    m_refreshPending = true;
    refreshResources();
}
