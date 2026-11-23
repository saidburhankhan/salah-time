#include "backend.h"

void Backend::loadBacklog(const QString &tagId, const QString &projectId) {
    QJsonObject params;
    if (!tagId.isEmpty()) params.insert(QStringLiteral("tag_id"), tagId);
    if (!projectId.isEmpty()) params.insert(QStringLiteral("project_id"), projectId);
    if (m_backlogParams != params) m_backlogItems.clear();
    m_backlogParams = params;
    m_backlogOpen = true;
    refreshBacklog();
}

void Backend::closeBacklog() {
    m_backlogOpen = false;
    ++m_backlogRequest;
    if (m_backlogLoading) {
        m_backlogLoading = false;
        emit backlogChanged();
    }
}

void Backend::refreshBacklog() {
    if (!m_backlogOpen || m_client.offline()) return;
    const quint64 request = ++m_backlogRequest;
    m_backlogLoading = true;
    emit backlogChanged();
    m_client.rpcRead(QStringLiteral("backlog.list"), m_backlogParams,
                     [this, request](const CoreClient::Result &result) {
        if (request != m_backlogRequest) return;
        m_backlogLoading = false;
        if (const auto *data = std::get_if<Dto::Json>(&result.value)) {
            m_backlogItems = data->value.toArray().toVariantList();
        } else if (!result.ok && !m_client.offline()) {
            fail(result.error.message);
        }
        emit backlogChanged();
    });
}

QVariantMap Backend::backlogItem(const QString &id) const {
    for (const QVariant &entry : m_backlogItems) {
        const QVariantMap row = entry.toMap();
        if (row.value(QStringLiteral("id")).toString() == id) return row;
    }
    return {};
}

qint64 Backend::backlogVersionFor(const QString &id) const {
    return backlogItem(id).value(QStringLiteral("version"), -1).toLongLong();
}

void Backend::moveBacklogItem(const QString &id, const QString &anchorId, bool before) {
    if (id.isEmpty() || anchorId.isEmpty() || id == anchorId || m_pendingItems.contains(id)) return;
    setItemPending(id, true);
    m_client.itemMove(id, {}, 0, mutationResponse(id), backlogVersionFor(id),
                      before ? QString() : anchorId, before ? anchorId : QString());
}

void Backend::scheduleItem(const QString &id, const QString &day, const QString &block) {
    if (id.isEmpty() || m_pendingItems.contains(id)) return;
    QJsonObject params{{"id", id}, {"day", day}, {"block", block}};
    const qint64 version = backlogVersionFor(id);
    if (version >= 0) params.insert(QStringLiteral("expected_version"), version);
    setItemPending(id, true);
    m_client.mutateEntity(QStringLiteral("item.schedule"), params,
                          [this, id, finish = mutationResponse(id)](const CoreClient::Result &result) {
        finish(result);
        emit scheduleFinished(id, result.ok, result.ok ? QString() : result.error.message);
    });
}

void Backend::sendToBacklog(const QString &id) {
    if (id.isEmpty() || m_pendingItems.contains(id)) return;
    QJsonObject params{{"id", id}};
    const qint64 version = versionFor(id);
    if (version >= 0) params.insert(QStringLiteral("expected_version"), version);
    setItemPending(id, true);
    m_client.mutateEntity(QStringLiteral("item.backlog"), params, mutationResponse(id));
}
