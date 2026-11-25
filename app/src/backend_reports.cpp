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
        if (!it.key().isEmpty() && (bounds.isEmpty() || !range.isEmpty())) ranges.insert(it.key(), range);
    }
    if (!ranges.isEmpty()) params.insert(QStringLiteral("values"), ranges);
    return params;
}

void Backend::loadReportItems() {
    if (m_client.offline()) {
        loadProjectCatalogues();
        m_client.tagsList(settingsListResponse(m_tags));
        m_client.metricsList(settingsListResponse(m_metrics));
    }
    const quint64 request = ++m_reportItemsRequest;
    m_client.itemsList(readResponse([this, request](const CoreClient::Result &result) {
        if (request != m_reportItemsRequest) return;
        if (const auto *data = std::get_if<Dto::Json>(&result.value); data && data->value.isArray()) {
            const QVariantList items = data->value.toArray().toVariantList();
            if (m_reportItems == items) return;
            m_reportItems = items;
            emit reportItemsChanged();
        }
    }), true, true);
}

void Backend::setRoutinesOpen(bool open) {
    m_routinesOpen = open;
    ++m_routineRequest;
    if (!open) return;
    if (m_client.offline()) loadProjectCatalogues();
    loadTrackingItems();
}

void Backend::loadRoutineHistory(const QDate &through, const QString &before) {
    if (before.isEmpty()) {
        ++m_routineRequest;
        m_pendingRoutineHistory.clear();
    }
    const quint64 request = m_routineRequest;
    const QDate from = through.addDays(1 - RoutinesModel::HistoryDays);
    m_client.routineActivity(from, through, before, [this, request, from, through, before](const CoreClient::Result &result) {
        if (request != m_routineRequest) return;
        const auto *data = std::get_if<Dto::Json>(&result.value);
        if (!result.ok || !data || !data->json.value("rows").isArray()) {
            m_routines.setHistory({}, {});
            m_endedRoutines.setHistory({}, {});
            m_pausedRoutines.setHistory({}, {});
            fail(result.ok ? tr("Could not read routine activity.") : result.error.message);
            return;
        }
        for (const auto &entry : data->json.value("rows").toArray()) {
            const auto row = entry.toObject();
            const QString id = row.value("item_id").toString();
            const QDate day = QDate::fromString(row.value("day").toString(), Qt::ISODate);
            if (id.isEmpty() || day < from || day > through) continue;
            auto &activity = m_pendingRoutineHistory[id];
            if (row.value("done").toBool()) activity.done.insert(day);
            activity.minutes += row.value("minutes").toInteger();
        }
        const QString next = data->json.value("next").toString();
        if (!next.isEmpty() && next != before) loadRoutineHistory(through, next);
        else {
            m_endedRoutines.setHistory(through, m_pendingRoutineHistory);
            m_pausedRoutines.setHistory(through, m_pendingRoutineHistory);
            m_routines.setHistory(through, std::move(m_pendingRoutineHistory));
        }
    });
}

void Backend::loadInsights(const QVariantMap &filter) {
    const quint64 request = ++m_insightsRequest;
    QJsonObject params = filterParams(filter, false);
    if (params.isEmpty()) {
        m_insights.clear();
        emit insightsChanged();
        return;
    }
    if (params.contains("days")) {
        const int days = params.take("days").toInt();
        params.insert("from", m_today.addDays(1 - days).toString(Qt::ISODate));
        params.insert("to", todayDay());
    }
    if (params != m_insightsParams) {
        m_insightsParams = params;
        m_insights.clear();
        emit insightsChanged();
    }
    m_client.insights(params, [this, request](const CoreClient::Result &result) {
        if (request != m_insightsRequest) return;
        if (result.ok) {
            m_insights = std::get<Dto::Json>(result.value).json.toVariantMap();
        } else {
            m_insights.clear();
            fail(result.error.message);
        }
        emit insightsChanged();
    });
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

void Backend::loadNotesReport(const QVariantMap &filter, const QString &query, const QString &kind) {
    const QJsonObject scope = filterParams(filter, true);
    if (scope.isEmpty()) return;
    auto params = scope;
    const auto text = query.trimmed();
    if (!text.isEmpty()) params.insert("q", text);
    if (kind == "main" || kind == "entry") params.insert("kind", kind);
    params.insert("limit", 50);
    if (m_notesParams != params) {
        m_noteRows.clear();
        m_notesTotal = 0;
    }
    m_notesParams = params;
    m_notesNext.clear();
    requestNotesReport({});
}

void Backend::loadMoreNotesReport() {
    if (!m_notesLoading && !m_notesNext.isEmpty()) requestNotesReport(m_notesNext);
}

void Backend::requestNotesReport(const QString &before) {
    auto params = m_notesParams;
    if (!before.isEmpty()) params.insert("before", before);
    const quint64 request = ++m_notesRequest;
    m_notesError.clear();
    m_notesLoading = true;
    emit notesReportChanged();

    m_client.notesList(params, [this, request, before](const CoreClient::Result &result) {
        if (request != m_notesRequest) return;
        m_notesLoading = false;
        const auto *data = std::get_if<Dto::Json>(&result.value);
        if (result.ok && data && data->json.value("rows").isArray()) {
            const auto rows = data->json.value("rows").toArray().toVariantList();
            if (before.isEmpty()) m_noteRows = rows;
            else m_noteRows.append(rows);
            m_notesTotal = data->json.value("total").toInteger();
            const auto next = data->json.value("next").toString();
            m_notesNext = next == before ? QString() : next;
        } else {
            m_notesError = result.ok ? tr("Could not read the notes list.") : result.error.message;
            if (before.isEmpty()) {
                m_noteRows.clear();
                m_notesTotal = 0;
            }
        }
        emit notesReportChanged();
    });
}

void Backend::loadSessionForNote(int request, const QString &id, const QVariantMap &context) {
    m_sessionForNoteRequest = request;
    const auto day = context.value("day").toString();
    if (id.isEmpty() || !QDate::fromString(day, Qt::ISODate).isValid()) {
        emit sessionForNoteLoaded(request, {}, tr("Could not locate the session."));
        return;
    }
    // The note catalogue carries a session summary. Read its current editor
    // fields, including the main note, through a bounded single-day query.
    QJsonObject params{{"from", day}, {"to", day}, {"limit", 10}, {"with_note", false}};
    const auto item = context.value("item_id").toString();
    if (!item.isEmpty()) params.insert("item_id", item);
    findSessionForNote(request, id, params);
}

void Backend::findSessionForNote(int request, const QString &id, QJsonObject params) {
    m_client.rpcRead(QStringLiteral("sessions.list"), params, [this, request, id, params](const CoreClient::Result &result) mutable {
        if (request != m_sessionForNoteRequest) return;
        if (const auto *data = std::get_if<Dto::Json>(&result.value); result.ok && data) {
            for (const auto &value : data->json.value("rows").toArray()) {
                const auto row = value.toObject();
                if (row.value("id").toString() != id) continue;
                emit sessionForNoteLoaded(request, row.toVariantMap(), {});
                return;
            }
            const auto next = data->json.value("next").toString();
            if (!next.isEmpty() && next != params.value("before").toString()) {
                params.insert("before", next);
                findSessionForNote(request, id, params);
                return;
            }
        }
        emit sessionForNoteLoaded(request, {}, result.ok ? tr("This session is no longer available.") : result.error.message);
    }, true);
}

void Backend::loadSessionNoteEntries(int request, const QString &id, const QString &after) {
    QJsonObject params{{"id", id}};
    if (!after.isEmpty()) params.insert("after", after);
    m_client.rpcRead(QStringLiteral("session.notes"), params, [this, request](const CoreClient::Result &result) {
        if (const auto *data = std::get_if<Dto::Json>(&result.value); result.ok && data && data->json.value("notes").isArray())
            emit sessionNoteEntriesLoaded(request, data->json.toVariantMap(), {});
        else
            emit sessionNoteEntriesLoaded(request, {}, result.ok ? tr("Could not read the timed notes.") : result.error.message);
    }, true);
}

void Backend::writeSessionNote(const QVariantMap &edit, const QString &operation) {
    const QString id = edit.value("sessionId").toString();
    const QString noteId = edit.value("noteId").toString();
    const QString text = edit.value("text").toString();
    const QString action = edit.value("action").toString();
    const qint64 version = edit.value("version", -1).toLongLong();
    if (id.isEmpty() || version < 0 || (action != "remove" && text.trimmed().isEmpty())) {
        emit sessionNoteWriteFinished(operation, false, false, {}, tr("A session version and a note are required."));
        return;
    }

    auto finished = [this, id, operation](const CoreClient::Result &result) {
        if (!result.ok) {
            const bool uncertain = !result.error.definitiveRejection() && result.error.kind != CoreClient::ErrorKind::Conflict;
            emit sessionNoteWriteFinished(operation, false, uncertain, {}, result.error.message);
            return;
        }
        const auto *mutation = std::get_if<Dto::MutationResult>(&result.value);
        if (mutation) {
            const auto sessions = mutation->json.value("changed").toObject().value("sessions").toArray();
            for (const auto &value : sessions) {
                const auto session = value.toObject();
                if (session.value("id").toString() != id || !session.value("version").isDouble()) continue;
                clearError();
                m_refreshPending = true;
                emit sessionNoteWriteFinished(operation, true, false, session.toVariantMap(), {});
                if (session.value("running").toBool() && m_running.value("id").toString() == id
                    && session.value("version").toInteger() >= m_running.value("version").toLongLong()) {
                    m_running.insert("version", session.value("version").toVariant());
                    m_running.insert("note", session.value("note").toVariant());
                    m_running.insert("notes", session.value("notes").toVariant());
                    refreshRunning();
                }
                emit sessionWriteFinished(true, {}, operation);
                return;
            }
        }
        emit sessionNoteWriteFinished(operation, false, true, {}, tr("Could not confirm the saved note."));
    };
    if (action == "main")
        m_client.sessionEdit(id, -1, false, {}, true, text, std::move(finished), version, operation);
    else if (action == "add")
        m_client.sessionNote(id, text, std::move(finished), edit.value("at").toString(), version, operation);
    else
        m_client.changeSessionNote(id, noteId, text, action == "remove", version, operation, std::move(finished));
}
