// Active goals and history share the same socket and CLI read paths.
#include "backend.h"

#include <QJsonArray>
#include <QJsonObject>

namespace {
QString text(const QVariantMap &fields, const char *key) { return fields.value(QLatin1String(key)).toString(); }
}

void Backend::loadGoals() {
    const quint64 request = ++m_goalsRequest;
    m_client.goalsList([this, request](const CoreClient::Result &result) {
        if (request != m_goalsRequest) return;
        if (const auto *data = std::get_if<Dto::Json>(&result.value)) {
            // A new goal gets a unique position. Every target edit inherits it,
            // even when intermediate versions were removed during their first period.
            QVariant selectedPosition;
            for (const QVariant &entry : m_goals) {
                const QVariantMap row = entry.toMap();
                if (row.value("id").toString() == m_goalHistoryId)
                    selectedPosition = row.value("position");
            }
            m_goals = data->value.toArray().toVariantList();
            emit goalsChanged();
            // An open history follows the goal's edit chain, so refresh it too.
            if (!m_goalHistoryId.isEmpty()) {
                QString activeId;
                for (const QVariant &entry : m_goals) {
                    const QVariantMap goal = entry.toMap();
                    const QString id = goal.value("id").toString();
                    if (id == m_goalHistoryId || goal.value("replaces_id").toString() == m_goalHistoryId
                        || (selectedPosition.isValid() && goal.value("position") == selectedPosition)) {
                        activeId = id;
                        break;
                    }
                }
                loadGoalHistory(activeId);
            }
        } else if (!result.ok) {
            fail(result.error.message);
        }
    });
}

void Backend::loadGoalHistory(const QString &id) {
    const bool changedId = m_goalHistoryId != id;
    m_goalHistoryId = id;
    const quint64 request = ++m_goalHistoryRequest;
    if (changedId || id.isEmpty()) {
        m_goalHistory.clear();
        emit goalHistoryChanged();
    }
    if (id.isEmpty()) return;
    // The longest span the daemon serves (730 days), ending today.
    m_client.goalsHistory(id, m_today.addDays(-729).toString(Qt::ISODate), todayDay(), [this, request, id](const CoreClient::Result &result) {
        if (request != m_goalHistoryRequest || id != m_goalHistoryId) return;
        if (const auto *data = std::get_if<Dto::Json>(&result.value)) {
            m_goalHistory = data->value.toArray().toVariantList();
            emit goalHistoryChanged();
        } else if (!result.ok) {
            fail(result.error.message);
        }
    });
}

qint64 Backend::goalVersion(const QString &id) const {
    for (const QVariant &entry : m_goals) {
        const QVariantMap row = entry.toMap();
        if (row.value(QStringLiteral("id")).toString() == id) return row.value(QStringLiteral("version"), -1).toLongLong();
    }
    return -1;
}

void Backend::addGoal(const QVariantMap &fields) {
    const double target = fields.value(QStringLiteral("target")).toDouble();
    if (!(target > 0)) {
        emit goalWriteFinished(false, tr("The target must be greater than zero."));
        return;
    }
    QJsonObject params{{QStringLiteral("period"), text(fields, "period")}, {QStringLiteral("target"), target}};
    // At most one scope; none means all work.
    for (const char *key : {"project_id", "epic_id", "item_id"})
        if (!text(fields, key).isEmpty()) {
            params.insert(QLatin1String(key), text(fields, key));
            break;
        }
    const QString metric = text(fields, "metric_id");
    if (!metric.isEmpty()) {
        params.insert(QStringLiteral("metric_id"), metric);
        const QString direction = text(fields, "direction");
        if (!direction.isEmpty()) params.insert(QStringLiteral("direction"), direction);
    }
    m_client.mutateEntity(QStringLiteral("goal.add"), params, goalResponse());
}

void Backend::editGoal(const QString &id, double target) {
    if (id.isEmpty()) return;
    if (!(target > 0)) {
        emit goalWriteFinished(false, tr("The target must be greater than zero."));
        return;
    }
    QJsonObject params{{QStringLiteral("id"), id}, {QStringLiteral("target"), target}};
    const qint64 version = goalVersion(id);
    if (version >= 0) params.insert(QStringLiteral("expected_version"), version);
    m_client.mutateEntity(QStringLiteral("goal.edit"), params, goalResponse());
}

void Backend::endGoal(const QString &id) {
    if (id.isEmpty()) return;
    QJsonObject params{{QStringLiteral("id"), id}};
    const qint64 version = goalVersion(id);
    if (version >= 0) params.insert(QStringLiteral("expected_version"), version);
    m_client.mutateEntity(QStringLiteral("goal.end"), params, goalResponse());
}

void Backend::removeGoal(const QString &id) {
    if (id.isEmpty()) return;
    QJsonObject params{{QStringLiteral("id"), id}};
    const qint64 version = goalVersion(id);
    if (version >= 0) params.insert(QStringLiteral("expected_version"), version);
    m_client.mutateEntity(QStringLiteral("goal.remove"), params, goalResponse());
}

CoreClient::Callback Backend::goalResponse() {
    return [this](const CoreClient::Result &result) {
        if (!result.ok) {
            fail(result.error.message);
            emit goalWriteFinished(false, result.error.message);
            // A conflict means the list is stale.
            if (result.error.kind == CoreClient::ErrorKind::Conflict) loadGoals();
            return;
        }
        clearError();
        emit goalWriteFinished(true, QString());
        loadGoals();
    };
}
