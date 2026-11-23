// Backend reads for the Statistics general table and the Notes page, and the
// shared filter they send. Split from backend.cpp to keep that file focused on
// the day plan and session writes.
#include "backend.h"

#include <QJsonArray>
#include <QJsonObject>
#include <QSettings>

namespace {
const QSet<QString> kTableGroups{QStringLiteral("week"), QStringLiteral("day"), QStringLiteral("project"),
                                 QStringLiteral("epic"), QStringLiteral("tag"), QStringLiteral("item")};

// A number from QML (int, double or numeric string); false when unset or not a
// finite number.
bool numberOf(const QVariant &value, double *out) {
    if (!value.isValid() || value.isNull()) return false;
    if (value.typeId() == QMetaType::QString && value.toString().trimmed().isEmpty()) return false;
    bool ok = false;
    const double number = value.toDouble(&ok);
    if (!ok || !qIsFinite(number)) return false;
    *out = number;
    return true;
}
} // namespace

QJsonObject Backend::filterParams(const QVariantMap &filter, bool allowAnyDays) {
    QJsonObject params;
    const QDate from = QDate::fromString(filter.value(QStringLiteral("from")).toString(), Qt::ISODate);
    const QDate to = QDate::fromString(filter.value(QStringLiteral("to")).toString(), Qt::ISODate);
    if (filter.contains("from") || filter.contains("to")) {
        if (!from.isValid() || !to.isValid() || from > to || from.daysTo(to) >= 730) {
            fail(tr("Choose a valid date range of 1 to 730 days."));
            return {};
        }
        params.insert(QStringLiteral("from"), from.toString(Qt::ISODate));
        params.insert(QStringLiteral("to"), to.toString(Qt::ISODate));
    } else {
        int days = filter.value(QStringLiteral("days"), 28).toInt();
        const bool preset = days == 7 || days == 28 || days == 90;
        if (!preset && !(allowAnyDays && days >= 1 && days <= 730)) days = 28;
        params.insert(QStringLiteral("days"), days);
    }
    for (const char *key : {"tag_id", "project_id", "epic_id", "item_id"}) {
        const QString id = filter.value(QLatin1String(key)).toString();
        if (!id.isEmpty()) params.insert(QLatin1String(key), id);
    }
    for (const char *key : {"minutes_min", "minutes_max"}) {
        double minutes = 0;
        if (numberOf(filter.value(QLatin1String(key)), &minutes) && minutes >= 0)
            params.insert(QLatin1String(key), qRound(minutes));
    }
    const QVariantMap values = filter.value(QStringLiteral("values")).toMap();
    QJsonObject ranges;
    for (auto it = values.cbegin(); it != values.cend(); ++it) {
        const QVariantMap bounds = it.value().toMap();
        QJsonObject range;
        double bound = 0;
        if (numberOf(bounds.value(QStringLiteral("min")), &bound)) range.insert(QStringLiteral("min"), bound);
        if (numberOf(bounds.value(QStringLiteral("max")), &bound)) range.insert(QStringLiteral("max"), bound);
        if (!it.key().isEmpty() && !range.isEmpty()) ranges.insert(it.key(), range);
    }
    if (!ranges.isEmpty()) params.insert(QStringLiteral("values"), ranges);
    return params;
}

void Backend::loadStatsQuery(const QVariantMap &filter) {
    const QJsonObject params = filterParams(filter, true);
    if (params.isEmpty()) return;
    auto apply = statsResponse();
    const quint64 request = m_statsRequest;
    m_client.statsQuery(params, [this, apply, params, request](const CoreClient::Result &result) {
        apply(result);
        if (!result.ok || request != m_statsRequest) return;
        // stats.get supplies overall focus and quality. The table supplies the
        // weighted range totals for other average metrics under the same filter.
        const auto series = m_stats.json.value("series").toArray();
        for (const auto &entry : series) {
            const auto metric = entry.toObject();
            if (metric.value("rollup") != "avg" || metric.value("metric_id") == "focus"
                || metric.value("metric_id") == "quality") continue;
            m_client.statsTable(params, [this, request](const CoreClient::Result &table) {
                if (request != m_statsRequest) return;
                if (!table.ok) { fail(table.error.message); return; }
                const auto totals = std::get<Dto::Json>(table.value).json.value("totals").toObject();
                m_stats.json.insert("range_values", totals.value("values"));
                emit statsDataChanged();
            });
            break;
        }
    });
}

void Backend::loadTrends(const QVariantMap &filter, const QStringList &measures,
                         const QString &interval, const QString &split) {
    const quint64 request = ++m_trendsRequest;
    if (measures.isEmpty()) {
        if (!m_trends.isEmpty()) {
            m_trends.clear();
            emit trendsChanged();
        }
        return;
    }
    QJsonObject params = filterParams(filter, true);
    if (params.isEmpty()) return;
    params.insert(QStringLiteral("measures"), QJsonArray::fromStringList(measures));
    params.insert(QStringLiteral("interval"), interval);
    if (!split.isEmpty()) params.insert(QStringLiteral("split"), split);
    m_client.statsSeries(params, [this, request](const CoreClient::Result &result) {
        if (request != m_trendsRequest) return;
        if (result.ok) {
            m_trends = std::get<Dto::Json>(result.value).json.toVariantMap();
        } else {
            m_trends.clear();
            fail(result.error.message);
        }
        emit trendsChanged();
    });
}

void Backend::loadStatsTable(const QVariantMap &filter, const QString &group) {
    // stats.table is served by the daemon only; offline there is nothing to show.
    if (m_client.offline()) {
        if (m_statsTable.isEmpty()) return;
        m_statsTable.clear();
        emit statsTableChanged();
        return;
    }
    QJsonObject params = filterParams(filter, true);
    if (params.isEmpty()) return;
    params.insert(QStringLiteral("group"), kTableGroups.contains(group) ? group : QStringLiteral("week"));
    const quint64 request = ++m_statsTableRequest;
    m_client.rpcRead(QStringLiteral("stats.table"), params, [this, request](const CoreClient::Result &result) {
        if (request != m_statsTableRequest) return;
        if (!result.ok) {
            // Never leave the previous query's rows under the new filter.
            m_statsTable.clear();
            emit statsTableChanged();
            fail(result.error.message);
            return;
        }
        if (const auto *json = std::get_if<Dto::Json>(&result.value)) {
            m_statsTable = json->json.toVariantMap();
            emit statsTableChanged();
        }
    });
}

void Backend::loadSessions(const QVariantMap &filter) {
    QJsonObject params = filterParams(filter, true);
    if (params.isEmpty()) return;
    const auto withNote = filter.constFind(QStringLiteral("with_note"));
    if (withNote != filter.cend()) params.insert(QStringLiteral("with_note"), withNote->toBool());
    const QString query = filter.value(QStringLiteral("q")).toString().trimmed();
    if (!query.isEmpty()) params.insert(QStringLiteral("q"), query);
    params.insert(QStringLiteral("limit"), 100);
    m_sessionsParams = params;
    requestSessions(params, false);
}

void Backend::loadMoreSessions() {
    if (m_sessionsNext.isEmpty() || m_sessionsLoading) return;
    QJsonObject params = m_sessionsParams;
    params.insert(QStringLiteral("before"), m_sessionsNext);
    requestSessions(params, true);
}

void Backend::requestSessions(const QJsonObject &params, bool append) {
    if (m_client.offline()) {
        m_sessionsList.clear();
        m_sessionsNext.clear();
        emit sessionsListChanged();
        return;
    }
    const quint64 request = ++m_sessionsRequest;
    if (!m_sessionsLoading) {
        m_sessionsLoading = true;
        emit sessionsLoadingChanged();
    }
    m_client.rpcRead(QStringLiteral("sessions.list"), params, [this, request, append](const CoreClient::Result &result) {
        if (request != m_sessionsRequest) return;
        m_sessionsLoading = false;
        emit sessionsLoadingChanged();
        if (!result.ok) {
            fail(result.error.message);
            return;
        }
        const auto *json = std::get_if<Dto::Json>(&result.value);
        if (!json) return;
        const QVariantList rows = json->json.value(QStringLiteral("rows")).toArray().toVariantList();
        if (append) m_sessionsList.append(rows);
        else m_sessionsList = rows;
        const QJsonValue next = json->json.value(QStringLiteral("next"));
        m_sessionsNext = next.isString() ? next.toString() : QString();
        emit sessionsListChanged();
    });
}

QVariant Backend::uiSetting(const QString &key, const QVariant &fallback) const {
    QSettings settings(QStringLiteral("salah"), QStringLiteral("salah-app"));
    return settings.value(key, fallback);
}

void Backend::setUiSetting(const QString &key, const QVariant &value) {
    QSettings settings(QStringLiteral("salah"), QStringLiteral("salah-app"));
    settings.setValue(key, value);
}
