#include "backend.h"

#include <QCalendar>
#include <QCoreApplication>
#include <QDateTime>
#include <QJsonArray>
#include <QJsonObject>
#include <QLocale>
#include <QTimer>
#include <algorithm>

namespace {
const QStringList kBlockOrder{QStringLiteral("night"), QStringLiteral("fajr"),
                              QStringLiteral("dhuhr"), QStringLiteral("asr"),
                              QStringLiteral("maghrib"), QStringLiteral("isha")};

QString blockName(const QString &block) {
    if (block == QStringLiteral("night")) return QCoreApplication::translate("Main", "Before Fajr");
    if (block == QStringLiteral("fajr")) return QCoreApplication::translate("Main", "Fajr");
    if (block == QStringLiteral("dhuhr")) return QCoreApplication::translate("Main", "Dhuhr");
    if (block == QStringLiteral("asr")) return QCoreApplication::translate("Main", "Asr");
    if (block == QStringLiteral("maghrib")) return QCoreApplication::translate("Main", "Maghrib");
    if (block == QStringLiteral("isha")) return QCoreApplication::translate("Main", "Isha");
    return block;
}

void sortItems(std::vector<Dto::DayItem> &items) {
    std::stable_sort(items.begin(), items.end(), [](const Dto::DayItem &a, const Dto::DayItem &b) {
        const int ap = a.json.value(QStringLiteral("position")).toInt();
        const int bp = b.json.value(QStringLiteral("position")).toInt();
        if (ap != bp) return ap < bp;
        return a.id < b.id;
    });
}

bool removeFromBlocks(std::vector<Dto::DayBlock> &blocks, const QString &id) {
    bool removed = false;
    for (Dto::DayBlock &block : blocks) {
        for (auto it = block.items.begin(); it != block.items.end();) {
            if (it->id == id) {
                it = block.items.erase(it);
                removed = true;
            } else {
                ++it;
            }
        }
    }
    return removed;
}

QStringList weekdaysOf(const QJsonObject &json) {
    QStringList days;
    for (const QJsonValue &day : json.value(QStringLiteral("weekdays")).toArray())
        days.append(day.toString());
    return days;
}

DayTimelineModel::Row itemRow(const Dto::DayItem &item, bool pending) {
    DayTimelineModel::Row row;
    row.kind = DayTimelineModel::Kind::Item;
    row.key = QStringLiteral("item:") + item.id;
    row.block = item.block;
    row.itemId = item.id;
    row.title = item.title;
    row.state = item.state;
    row.measure = item.measure;
    row.repeat = item.repeat;
    row.detailText = item.detail_text;
    row.scheduleText = item.schedule_text;
    row.nextOccurrence = item.next_occurrence;
    row.carried = item.carried;
    row.pending = pending;
    row.timerRunning = item.timer_running;
    row.timerStartedAt = item.timer_started_at;
    row.timerMinutes = int(item.timer_minutes_including_running);
    row.timerProjected = item.json.contains(QStringLiteral("timer"));
    row.count = item.json.value(QStringLiteral("count")).toInt();
    row.target = item.json.value(QStringLiteral("target")).toInt();
    row.weekdays = weekdaysOf(item.json);
    row.version = item.json.value(QStringLiteral("version")).toVariant().toLongLong();
    row.position = item.json.value(QStringLiteral("position")).toInt();
    const QJsonObject project = item.json.value(QStringLiteral("project")).toObject();
    row.projectColor = project.value(QStringLiteral("color")).toString();
    row.projectId = project.value(QStringLiteral("id")).toString();
    row.projectName = project.value(QStringLiteral("name")).toString();
    row.plannedMinutes = item.json.value(QStringLiteral("planned_minutes")).toInt();
    row.actualMinutes = item.json.value(QStringLiteral("actual_minutes")).toInt();
    const QJsonArray sessions = item.json.value(QStringLiteral("sessions")).toArray();
    row.sessions = sessions.size();
    row.sessionList = sessions.toVariantList();
    const QJsonObject resources = item.json.value(QStringLiteral("resources")).toObject();
    row.resourceCount = resources.value(QStringLiteral("notes")).toInt()
        + resources.value(QStringLiteral("links")).toInt() + resources.value(QStringLiteral("reminders")).toInt();
    const QJsonValue metrics = item.json.value(QStringLiteral("metrics"));
    if (metrics.isArray()) row.promptMetrics = metrics.toArray().toVariantList();
    for (const QJsonValue &value : sessions) {
        const QJsonObject session = value.toObject();
        if (session.value(QStringLiteral("running")).toBool()) {
            row.runningSessionId = session.value(QStringLiteral("id")).toString();
            row.timerRunning = true;
            row.timerStartedAt = session.value(QStringLiteral("started_at")).toString();
        } else if (session.value(QStringLiteral("minutes")).isNull()) {
            row.ticked = true;
        }
    }
    return row;
}
} // namespace

Backend::Backend(QObject *parent, const QString &program, int timeoutMs, const QString &socketPath)
    : QObject(parent), m_client(this, program, timeoutMs, socketPath) {
    m_statsDailyNewest.setSourceModel(&m_statsDaily);
    m_statsDailyNewest.setSortRole(StatsDailyModel::DayRole);
    m_statsDailyNewest.setDynamicSortFilter(true);
    m_statsDailyNewest.sort(0, Qt::DescendingOrder);
    auto notifyCount = [this] { emit statsDailyNewestCountChanged(); };
    connect(&m_statsDailyNewest, &QAbstractItemModel::rowsInserted, this, notifyCount);
    connect(&m_statsDailyNewest, &QAbstractItemModel::rowsRemoved, this, notifyCount);
    connect(&m_statsDailyNewest, &QAbstractItemModel::modelReset, this, notifyCount);
    connect(&m_statsDailyNewest, &QAbstractItemModel::layoutChanged, this, notifyCount);
    m_editItem = new ItemDraft(this);

    connect(&m_client, &CoreClient::busyChanged, this, &Backend::busyChanged);
    connect(&m_client, &CoreClient::offlineChanged, this, [this](bool) { emit offlineChanged(); });
    connect(&m_client, &CoreClient::reconnected, this, [this] { reload(); });
    connect(&m_client, &CoreClient::changed, this, &Backend::reloadFromNotification);
    connect(&m_client, &CoreClient::requestFinished, this, [this](const QStringList &args, bool success) {
        emit requestFinished(args, success);
        // SocketTransport emits requestFinished before it removes the request
        // from its pending map. Recheck on the next event-loop turn so a
        // successful mutation can issue its authoritative day.get refresh.
        QTimer::singleShot(0, this, [this] {
            if (m_client.busy() || !m_refreshPending) return;
            m_refreshPending = false;
            reload();
        });
    });
}

Backend::~Backend() = default;

void Backend::fail(const QString &message) {
    if (m_error == message) return;
    m_error = message;
    emit errorChanged();
}

void Backend::clearError() {
    if (m_error.isEmpty()) return;
    m_error.clear();
    emit errorChanged();
}

void Backend::dismissError() { clearError(); }

CoreClient::Callback Backend::readResponse(std::function<void(const CoreClient::Result &)> success,
                                           bool reloadAfter) {
    return [this, success = std::move(success), reloadAfter](const CoreClient::Result &result) {
        if (!result.ok) fail(result.error.message);
        else {
            if (success) success(result);
            if (reloadAfter) m_refreshPending = true;
        }
    };
}

CoreClient::Callback Backend::mutationResponse(const QString &pendingId, const QString &pendingOpId) {
    return [this, pendingId, pendingOpId](const CoreClient::Result &result) {
        if (result.ok) {
            // Mutation changed rows are invalidations, not day projections.
            // In particular changed.items are ItemView and do not carry the
            // day snapshot fields. Clear optimistic state and refetch once.
            if (!pendingOpId.isEmpty()) removePendingCreate(pendingOpId);
            if (pendingId.startsWith(QStringLiteral("prayer:"))) setPrayerPending(pendingId.mid(7), false);
            else if (!pendingId.isEmpty()) setItemPending(pendingId, false);
            m_refreshPending = true;
        } else {
            if (!pendingOpId.isEmpty()) removePendingCreate(pendingOpId);
            if (pendingId.startsWith(QStringLiteral("prayer:"))) setPrayerPending(pendingId.mid(7), false);
            else if (!pendingId.isEmpty()) setItemPending(pendingId, false);
            fail(result.error.message);
            if (result.error.kind == CoreClient::ErrorKind::Conflict || !m_client.offline()) reload();
        }
    };
}

void Backend::reload() {
    m_client.day(m_selected, readResponse([this](const CoreClient::Result &result) {
        if (const auto *plan = std::get_if<Dto::DayPlan>(&result.value)) applyDayPlan(*plan);
    }));
    m_client.statsDays(14, readResponse([this](const CoreClient::Result &result) {
        if (const auto *stats = std::get_if<Dto::Stats>(&result.value)) applyRoutines(*stats);
    }));
    m_client.configShow(readResponse([this](const CoreClient::Result &result) {
        if (const auto *config = std::get_if<Dto::Config>(&result.value)) applyConfig(*config);
    }));
    // The running session (possibly one with only a project) and the metrics
    // its stop asks for.
    m_client.status(readResponse([this](const CoreClient::Result &result) {
        if (const auto *json = std::get_if<Dto::Json>(&result.value)) applyStatus(json->json);
    }));
    refreshBacklog();
}

void Backend::applyStatus(const QJsonObject &status) {
    const QJsonValue running = status.value(QStringLiteral("running"));
    m_running = running.isObject() ? running.toObject().toVariantMap() : QVariantMap();
    m_runningPrompt = running.isObject() ? status.value(QStringLiteral("prompt")).toArray().toVariantList()
                                         : QVariantList();
    refreshRunning();
}

void Backend::refreshRunning() {
    QString title;
    QString color;
    const QString itemId = m_running.value(QStringLiteral("item_id")).toString();
    const QString projectId = m_running.value(QStringLiteral("project_id")).toString();
    if (!itemId.isEmpty()) {
        for (const Dto::DayBlock &block : m_dayPlan.blocks)
            for (const Dto::DayItem &item : block.items)
                if (item.id == itemId) {
                    title = item.title;
                    color = item.json.value(QStringLiteral("project")).toObject().value(QStringLiteral("color")).toString();
                }
    }
    for (const QVariant &value : std::as_const(m_projects)) {
        const QVariantMap project = value.toMap();
        if (projectId.isEmpty() || project.value(QStringLiteral("id")).toString() != projectId) continue;
        if (title.isEmpty()) title = project.value(QStringLiteral("name")).toString();
        if (color.isEmpty()) color = project.value(QStringLiteral("color")).toString();
    }
    if (title.isEmpty() && !m_running.isEmpty()) title = tr("Session");
    if (title == m_runningTitle && color == m_runningColor && m_running == m_lastRunning
        && m_runningPrompt == m_lastRunningPrompt)
        return;
    m_runningTitle = title;
    m_runningColor = color;
    m_lastRunning = m_running;
    m_lastRunningPrompt = m_runningPrompt;
    emit runningChanged();
}

void Backend::reloadFromNotification(qint64 revision, const QStringList &scopes) {
    if (revision <= m_lastNotificationRevision) return;
    m_lastNotificationRevision = revision;
    reload();
    if (scopes.isEmpty() || scopes.contains("resources")) refreshResources();
    // Another surface (the bar widget, the CLI) may have changed projects,
    // tags, or metrics; the lists are cheap.
    loadSettings();
    if (scopes.isEmpty() || scopes.contains("reviews") || scopes.contains("sessions") || scopes.contains("goals")
        || scopes.contains("items"))
        emit reviewsInvalidated();
    if (scopes.isEmpty() || scopes.contains("sessions") || scopes.contains("items")
        || scopes.contains("projects") || scopes.contains("epics") || scopes.contains("tags")
        || scopes.contains("metrics") || scopes.contains("config") || scopes.contains("marks"))
        emit reportsInvalidated();
}

void Backend::applyDayPlan(const Dto::DayPlan &plan) {
    const QString previousSelected = selectedDay();
    const QString previousToday = todayDay();
    m_dayPlan = plan;
    emit fulfilmentChanged();
    m_dayPlanFetchedAt = QDateTime::currentMSecsSinceEpoch();
    const QDate cliDay = QDate::fromString(plan.day, Qt::ISODate);
    if (cliDay.isValid()) {
        m_selected = cliDay;
        if (plan.json.value(QStringLiteral("is_today")).toBool()) m_today = cliDay;
    }
    m_versions.clear();
    for (const Dto::DayBlock &block : m_dayPlan.blocks)
        for (const Dto::DayItem &item : block.items)
            if (item.json.contains(QStringLiteral("version")))
                m_versions.insert(item.id, item.json.value(QStringLiteral("version")).toVariant().toLongLong());

    rebuildTimeline();
    rebuildPrayers();
    m_now.update(m_dayPlan, m_dayPlanFetchedAt);
    refreshRunning();
    if (previousSelected != selectedDay()) emit selectedDayChanged();
    if (previousToday != todayDay()) emit todayDayChanged();
}

void Backend::applyStats(const Dto::Stats &stats) {
    m_stats = stats;
    emit statsDataChanged();
    m_totals.update(m_stats);
    m_statsItems.setItems(m_stats.items, StatsItemsModel::Sort::MinutesDescending);
    m_ticks.setTicks(m_stats.time_ticks);
    m_statsDaily.setStats(m_stats, todayDay(), m_selectedStatsItem);
}

void Backend::applyRoutines(const Dto::Stats &stats) {
    m_routinesStats = stats;
    m_routines.setStats(m_routinesStats);
}

void Backend::applyConfig(const Dto::Config &config) {
    m_config = config;
    m_configState.update(m_config);
}

void Backend::rebuildTimeline() {
    QVector<DayTimelineModel::Row> rows;
    QHash<QString, const Dto::DayBlock *> blockFor;
    for (const Dto::DayBlock &block : m_dayPlan.blocks) blockFor.insert(block.start, &block);
    QHash<QString, const Dto::Prayer *> prayerFor;
    for (const Dto::Prayer &prayer : m_dayPlan.prayers) prayerFor.insert(prayer.id, &prayer);
    const QString nowBlock = m_dayPlan.now ? m_dayPlan.now->block : QString();
    const QString nightStart = m_dayPlan.json.value(QStringLiteral("night_start")).toString();
    const QString sunrise = m_dayPlan.json.value(QStringLiteral("sunrise")).toString();

    for (const QString &blockId : kBlockOrder) {
        const Dto::Prayer *prayer = prayerFor.value(blockId, nullptr);
        DayTimelineModel::Row header;
        header.kind = DayTimelineModel::Kind::Header;
        header.key = QStringLiteral("header:") + blockId;
        header.block = blockId;
        header.name = blockName(blockId);
        header.time = blockId == QStringLiteral("night")
            ? nightStart
            : (prayer ? prayer->json.value(QStringLiteral("time")).toString() : QString());
        header.marked = prayer && prayer->json.value(QStringLiteral("marked")).toBool();
        header.current = blockId == nowBlock;
        const Dto::DayBlock *capacityBlock = blockFor.value(blockId, nullptr);
        if (capacityBlock) {
            header.capacityMinutes = capacityBlock->json.value(QStringLiteral("capacity_minutes")).toInt();
            header.blockPlannedMinutes = capacityBlock->json.value(QStringLiteral("planned_minutes")).toInt();
            header.blockActualMinutes = capacityBlock->json.value(QStringLiteral("actual_minutes")).toInt();
        }
        rows.append(header);

        const Dto::DayBlock *block = blockFor.value(blockId, nullptr);
        QVector<DayTimelineModel::Row> itemRows;
        if (block) {
            std::vector<Dto::DayItem> items = block->items;
            sortItems(items);
            for (const Dto::DayItem &item : items)
                itemRows.append(itemRow(item, m_pendingItems.contains(item.id)));
        }
        for (const PendingCreate &pending : m_pendingCreates) {
            if (pending.block != blockId) continue;
            DayTimelineModel::Row row;
            row.kind = DayTimelineModel::Kind::Item;
            row.key = QStringLiteral("item:") + pending.localId;
            row.block = pending.block;
            row.itemId = pending.localId;
            row.title = pending.title;
            row.state = QStringLiteral("open");
            row.measure = QStringLiteral("check");
            row.repeat = pending.repeat;
            row.target = pending.plannedMinutes;
            row.weekdays = pending.weekdays;
            row.pending = true;
            row.position = 1000000 + itemRows.size();
            itemRows.append(row);
        }
        for (const DayTimelineModel::Row &row : itemRows) rows.append(row);

        DayTimelineModel::Row add;
        add.kind = DayTimelineModel::Kind::Add;
        add.key = QStringLiteral("add:") + blockId;
        add.block = blockId;
        rows.append(add);

        if (blockId == QStringLiteral("fajr")) {
            DayTimelineModel::Row marker;
            marker.kind = DayTimelineModel::Kind::Marker;
            marker.key = QStringLiteral("marker:sunrise");
            marker.block = blockId;
            marker.name = QCoreApplication::translate("Main", "Sunrise");
            marker.time = sunrise;
            rows.append(marker);
        }
        if (blockId == QStringLiteral("isha")) {
            DayTimelineModel::Row marker;
            marker.kind = DayTimelineModel::Kind::Marker;
            marker.key = QStringLiteral("marker:sleep");
            marker.block = blockId;
            marker.name = QCoreApplication::translate("Main", "Sleep");
            marker.time = m_dayPlan.sleep.time;
            rows.append(marker);
        }
    }
    m_timeline.setRows(rows);
}

void Backend::rebuildPrayers() {
    QVector<PrayerModel::Row> rows;
    rows.reserve(int(m_dayPlan.prayers.size()));
    for (const Dto::Prayer &prayer : m_dayPlan.prayers) {
        PrayerModel::Row row;
        row.prayerId = prayer.id;
        row.name = prayer.json.value(QStringLiteral("name")).toString();
        row.time = prayer.json.value(QStringLiteral("time")).toString();
        row.passed = prayer.json.value(QStringLiteral("passed")).toBool();
        row.current = prayer.json.value(QStringLiteral("current")).toBool();
        row.marked = prayer.json.value(QStringLiteral("marked")).toBool();
        row.pending = m_pendingPrayers.contains(prayer.id);
        row.version = versionFor(prayer.id);
        row.day = m_dayPlan.day;
        rows.append(row);
    }
    m_prayers.setRows(rows);
}

void Backend::setItemPending(const QString &id, bool pending) {
    if (pending) m_pendingItems.insert(id);
    else m_pendingItems.remove(id);
    rebuildTimeline();
}

void Backend::setPrayerPending(const QString &prayer, bool pending) {
    if (pending) m_pendingPrayers.insert(prayer);
    else m_pendingPrayers.remove(prayer);
    rebuildPrayers();
}

void Backend::insertPending(const PendingCreate &pending) {
    m_pendingCreates.insert(pending.opId, pending);
    m_pendingItems.insert(pending.localId);
    rebuildTimeline();
}

void Backend::removePendingCreate(const QString &opId) {
    const auto it = m_pendingCreates.find(opId);
    if (it == m_pendingCreates.end()) return;
    const QString id = it.value().localId;
    m_pendingCreates.erase(it);
    m_pendingItems.remove(id);
    rebuildTimeline();
}

qint64 Backend::versionFor(const QString &id) const {
    const auto it = m_versions.constFind(id);
    return it == m_versions.constEnd() ? -1 : it.value();
}

bool Backend::dayContains(const QString &id) const {
    for (const Dto::DayBlock &block : m_dayPlan.blocks)
        for (const Dto::DayItem &item : block.items)
            if (item.id == id) return true;
    return false;
}

void Backend::applyMutation(const Dto::MutationResult &mutation) {
    removePendingCreate(mutation.opId);
    for (const QString &id : mutation.deleted) {
        removeFromBlocks(m_dayPlan.blocks, id);
        m_pendingItems.remove(id);
        m_versions.remove(id);
    }
    for (const Dto::DayItem &row : mutation.items) {
        if (row.json.contains(QStringLiteral("version")))
            m_versions.insert(row.id, row.json.value(QStringLiteral("version")).toVariant().toLongLong());
        m_pendingItems.remove(row.id);
        removeFromBlocks(m_dayPlan.blocks, row.id);
        for (Dto::DayBlock &block : m_dayPlan.blocks) {
            if (block.start != row.block) continue;
            block.items.push_back(row);
            sortItems(block.items);
            break;
        }
    }
    for (const Dto::PrayerMark &mark : mutation.marks) {
        if (mark.json.contains(QStringLiteral("version")))
            m_versions.insert(mark.id, mark.json.value(QStringLiteral("version")).toVariant().toLongLong());
        m_versions.insert(QStringLiteral("%1:%2").arg(mark.day, mark.prayer),
                          mark.json.value(QStringLiteral("version")).toVariant().toLongLong());
        m_pendingPrayers.remove(mark.prayer);
        for (Dto::Prayer &prayer : m_dayPlan.prayers) {
            if (prayer.id != mark.prayer) continue;
            prayer.json[QStringLiteral("marked")] =
                mark.json.value(QStringLiteral("deleted_at")).isNull() ||
                !mark.json.contains(QStringLiteral("deleted_at"));
            break;
        }
    }
    m_dayPlanFetchedAt = QDateTime::currentMSecsSinceEpoch();
    rebuildTimeline();
    rebuildPrayers();
    m_now.update(m_dayPlan, m_dayPlanFetchedAt);
}

void Backend::loadStats(int days) {
    loadStatsFiltered(days, {}, {});
}

void Backend::loadStatsFiltered(int days, const QString &tagId, const QString &projectId, const QString &itemId) {
    if (days != 7 && days != 28 && days != 90) days = 28;
    m_client.statsFiltered(days, tagId, projectId, itemId, statsResponse());
}

void Backend::loadStatsRange(const QString &from, const QString &to, const QString &tagId,
                             const QString &projectId, const QString &itemId) {
    const QDate first = QDate::fromString(from, Qt::ISODate);
    const QDate last = QDate::fromString(to, Qt::ISODate);
    if (!first.isValid() || !last.isValid() || first > last || first.daysTo(last) >= 730) {
        fail(tr("Invalid statistics date range."));
        return;
    }
    m_client.statsFiltered(0, tagId, projectId, itemId, statsResponse(), first, last);
}

CoreClient::Callback Backend::statsResponse() {
    const quint64 request = ++m_statsRequest;
    return [this, request](const CoreClient::Result &result) {
        if (request != m_statsRequest) return;
        if (!result.ok) {
            applyStats({});
            fail(result.error.message);
        } else if (const auto *stats = std::get_if<Dto::Stats>(&result.value)) applyStats(*stats);
    };
}

void Backend::loadTrackingItems() {
    m_client.itemsList(readResponse([this](const CoreClient::Result &result) {
        if (const auto *json = std::get_if<Dto::Json>(&result.value)) {
            const QVariantList items = json->value.toArray().toVariantList();
            if (m_trackingItems == items) return;
            m_trackingItems = items;
            emit trackingItemsChanged();
        }
    }));
}

void Backend::moveDay(int amount) {
    if (!m_selected.isValid()) m_selected = m_today;
    m_selected = m_selected.addDays(amount);
    emit selectedDayChanged();
    reload();
}

void Backend::selectDay(const QString &day) {
    const QDate parsed = QDate::fromString(day, Qt::ISODate);
    if (parsed.isValid() && parsed != m_selected) {
        m_selected = parsed;
        emit selectedDayChanged();
        reload();
    }
}

void Backend::jumpToday() {
    m_selected = m_today;
    emit selectedDayChanged();
    reload();
}

void Backend::parseQuick(const QString &text) {
    if (text.trimmed().isEmpty()) {
        m_parse = Dto::ParseDraft{};
        m_parseDraft.clear();
        return;
    }
    m_client.parse(text, readResponse([this](const CoreClient::Result &result) {
        if (const auto *draft = std::get_if<Dto::ParseDraft>(&result.value)) {
            m_parse = *draft;
            m_parseDraft.update(m_parse);
        }
    }));
}

void Backend::previewItem(const QVariantMap &item) {
    const QString repeat = item.value(QStringLiteral("repeat"), QStringLiteral("none")).toString();
    QStringList weekdays = item.value(QStringLiteral("weekdays")).toStringList();
    if (repeat == QStringLiteral("weekly") && weekdays.isEmpty()) {
        m_preview.clear();
        return;
    }
    CoreClient::ItemFields fields;
    fields.block = item.value(QStringLiteral("block"), QStringLiteral("fajr")).toString();
    if (fields.block.isEmpty() || fields.block == QStringLiteral("sleep")) fields.block = QStringLiteral("fajr");
    fields.repeat = repeat;
    fields.weekdays = weekdays;
    fields.projectId = item.value(QStringLiteral("project_id")).toString();
    fields.plannedMinutes = item.value(QStringLiteral("planned_minutes")).toInt();
    fields.hasPlannedMinutes = fields.plannedMinutes > 0;
    fields.date = m_selected.isValid() ? m_selected : m_today;
    m_client.preview(fields, [this](const CoreClient::Result &result) {
        if (result.ok) {
            if (const auto *preview = std::get_if<Dto::Preview>(&result.value)) m_preview.update(*preview);
            else m_preview.clear();
        } else {
            m_preview.clear();
        }
    });
}

void Backend::addQuick(const QString &text, const QString &block) {
    CoreClient::ItemFields fields;
    fields.title = text;
    fields.block = block.isEmpty() ? QStringLiteral("fajr") : block;
    fields.quick = true;
    fields.opId = CoreClient::makeOpId();
    if (m_selected.isValid() && m_selected != m_today) fields.date = m_selected;
    PendingCreate pending;
    pending.opId = fields.opId;
    pending.localId = QStringLiteral("pending-%1").arg(fields.opId);
    pending.title = text;
    pending.block = fields.block;
    insertPending(pending);
    m_client.itemAdd(fields, mutationResponse(pending.localId, fields.opId));
}

void Backend::saveItem(const QVariantMap &item) {
    const QString id = item.value(QStringLiteral("id")).toString();
    CoreClient::ItemFields fields;
    fields.title = item.value(QStringLiteral("title")).toString();
    fields.backlog = item.value(QStringLiteral("backlog")).toBool();
    fields.block = item.value(QStringLiteral("block"), QStringLiteral("fajr")).toString();
    fields.repeat = item.value(QStringLiteral("repeat"), QStringLiteral("none")).toString();
    fields.weekdays = item.value(QStringLiteral("weekdays")).toStringList();
    fields.projectId = item.value(QStringLiteral("project_id")).toString();
    // epic_id is present only when it is set or being cleared; `null` clears.
    const auto epic = item.constFind(QStringLiteral("epic_id"));
    if (epic != item.cend()) fields.epicId = epic->toString().isEmpty() ? QJsonValue(QJsonValue::Null)
                                                                        : QJsonValue(epic->toString());
    const auto metricIds = item.constFind(QStringLiteral("metric_ids"));
    if (metricIds != item.cend()) fields.metricIds = QJsonValue::fromVariant(*metricIds);
    const auto tagIds = item.constFind(QStringLiteral("tag_ids"));
    if (tagIds != item.cend()) fields.tagIds = QJsonValue::fromVariant(*tagIds);
    fields.plannedMinutes = item.value(QStringLiteral("planned_minutes")).toInt();
    fields.hasPlannedMinutes = fields.plannedMinutes > 0;
    if (id.isEmpty()) {
        if (fields.backlog) {
            m_client.itemAdd(fields, mutationResponse());
            return;
        }
        if (fields.repeat == QStringLiteral("none") && m_selected != m_today) fields.date = m_selected;
        fields.opId = CoreClient::makeOpId();
        PendingCreate pending;
        pending.opId = fields.opId;
        pending.localId = QStringLiteral("pending-%1").arg(fields.opId);
        pending.title = fields.title;
        pending.block = fields.block;
        pending.repeat = fields.repeat;
        pending.projectId = fields.projectId;
        pending.plannedMinutes = fields.hasPlannedMinutes ? fields.plannedMinutes : 0;
        pending.weekdays = fields.weekdays;
        insertPending(pending);
        m_client.itemAdd(fields, mutationResponse(pending.localId, fields.opId));
    } else {
        setItemPending(id, true);
        m_client.itemEdit(id, fields, mutationResponse(id),
                          fields.backlog ? backlogVersionFor(id) : versionFor(id));
    }
}

void Backend::removeItem(const QString &id) {
    setItemPending(id, true);
    m_client.itemRemove(id, mutationResponse(id), m_backlogOpen ? backlogVersionFor(id) : versionFor(id));
}

void Backend::optimisticMove(const QString &id, const QString &block, const QString &afterId,
                             const QString &beforeId) {
    Dto::DayItem moving;
    bool found = false;
    for (Dto::DayBlock &entry : m_dayPlan.blocks) {
        for (auto it = entry.items.begin(); it != entry.items.end(); ++it) {
            if (it->id != id) continue;
            moving = *it;
            entry.items.erase(it);
            found = true;
            break;
        }
        if (found) break;
    }
    if (!found) return;
    moving.block = block;
    moving.json[QStringLiteral("block")] = block;
    for (Dto::DayBlock &entry : m_dayPlan.blocks) {
        if (entry.start != block) continue;
        int insertAt = int(entry.items.size());
        for (int i = 0; i < int(entry.items.size()); ++i) {
            if (!afterId.isEmpty() && entry.items.at(i).id == afterId) { insertAt = i + 1; break; }
            if (!beforeId.isEmpty() && entry.items.at(i).id == beforeId) { insertAt = i; break; }
        }
        entry.items.insert(entry.items.begin() + insertAt, moving);
        for (int i = 0; i < int(entry.items.size()); ++i)
            entry.items[i].json[QStringLiteral("position")] = i;
        break;
    }
}

void Backend::moveItem(const QString &id, const QString &block, const QString &afterId,
                       const QString &beforeId) {
    optimisticMove(id, block, afterId, beforeId);
    setItemPending(id, true);
    m_client.itemMove(id, block, 0, mutationResponse(id), versionFor(id), afterId, beforeId);
}

QJsonArray Backend::sessionsOf(const QString &itemId) const {
    for (const Dto::DayBlock &block : m_dayPlan.blocks)
        for (const Dto::DayItem &item : block.items)
            if (item.id == itemId) return item.json.value(QStringLiteral("sessions")).toArray();
    return {};
}

QDate Backend::writeDate() const {
    return m_selected.isValid() && m_selected != m_today ? m_selected : QDate();
}

namespace {
bool isTick(const QJsonObject &session) {
    return !session.value(QStringLiteral("running")).toBool() && session.value(QStringLiteral("minutes")).isNull();
}
} // namespace

bool Backend::toggleDone(const QString &id, const QString &state) {
    Q_UNUSED(state)
    for (const QJsonValue &value : sessionsOf(id)) {
        if (isTick(value.toObject())) {
            untickItem(id);
            return true;
        }
    }
    tickItem(id);
    return true;
}

void Backend::tickItem(const QString &id, const QVariantMap &values, const QString &note, const QString &operation) {
    for (const QJsonValue &value : sessionsOf(id)) {
        if (isTick(value.toObject())) {
            const bool saved = values.isEmpty() && note.trimmed().isEmpty();
            emit sessionWriteFinished(saved, saved ? QString() : tr("Already marked done. Your ratings and note were not saved."), operation);
            return;
        }
    }
    setItemPending(id, true);
    // The server stops a matching timer and records the tick in one transaction.
    m_client.sessionLog(id, {}, -1, QJsonObject::fromVariantMap(values), note, writeDate(),
                        sessionResponse(id), {}, operation);
}

void Backend::untickItem(const QString &id) {
    QStringList ticks;
    for (const QJsonValue &value : sessionsOf(id)) {
        const QJsonObject session = value.toObject();
        if (isTick(session)) ticks.append(session.value(QStringLiteral("id")).toString());
    }
    if (ticks.isEmpty()) return;
    setItemPending(id, true);
    for (const QString &sessionId : ticks) m_client.sessionRemove(sessionId, sessionResponse(id));
}

void Backend::logSession(const QString &itemId, const QString &projectId, int minutes,
                         const QVariantMap &values, const QString &note, const QString &epicId,
                         const QString &day, const QString &operation) {
    if (itemId.isEmpty() && projectId.isEmpty() && epicId.isEmpty()) {
        emit sessionWriteFinished(false, tr("Choose a task, project or epic."), operation);
        return;
    }
    if (!itemId.isEmpty()) setItemPending(itemId, true);
    const bool epicOnly = itemId.isEmpty() && !epicId.isEmpty();
    m_client.sessionLog(itemId, itemId.isEmpty() && !epicOnly ? projectId : QString(), minutes > 0 ? minutes : -1,
                        QJsonObject::fromVariantMap(values), note, QDate::fromString(day, Qt::ISODate), sessionResponse(itemId),
                        epicOnly ? epicId : QString(), operation);
}

void Backend::stopSession(const QString &sessionId, const QVariantMap &values,
                          const QString &note, bool done, int minutes, const QString &operation) {
    const QString itemId = m_running.value(QStringLiteral("id")).toString() == sessionId || sessionId.isEmpty()
        ? m_running.value(QStringLiteral("item_id")).toString() : QString();
    if (!itemId.isEmpty()) setItemPending(itemId, true);
    m_client.sessionStop(sessionId, QJsonObject::fromVariantMap(values), note,
                         sessionResponse(itemId), done, minutes, operation);
}

void Backend::editSession(const QString &sessionId, int minutes, const QVariantMap &values, const QString &note,
                          qint64 version, const QString &operation) {
    m_client.sessionEdit(sessionId, minutes > 0 ? minutes : -1, true, QJsonObject::fromVariantMap(values),
                         true, note, sessionResponse({}), version, operation);
}

void Backend::removeSession(const QString &sessionId) {
    m_client.sessionRemove(sessionId, sessionResponse({}));
}

void Backend::bumpDistractions(const QString &sessionId, int delta) {
    m_client.sessionBump(sessionId, QStringLiteral("distractions"), delta,
                         [this](const CoreClient::Result &result) { applyRunningWrite(result); });
}

void Backend::addRunningNote(const QString &sessionId, const QString &text) {
    m_client.sessionNote(sessionId, text, [this, sessionId](const CoreClient::Result &result) {
        applyRunningWrite(result);
        emit runningNoteFinished(sessionId, result.ok);
    });
}

void Backend::applyRunningWrite(const CoreClient::Result &result) {
    if (const auto *mutation = std::get_if<Dto::MutationResult>(&result.value)) {
        const QJsonArray sessions = mutation->json.value(QStringLiteral("changed")).toObject()
                                        .value(QStringLiteral("sessions")).toArray();
        const QString runningId = m_running.value(QStringLiteral("id")).toString();
        for (const QJsonValue &value : sessions) {
            const QJsonObject session = value.toObject();
            if (session.value(QStringLiteral("running")).toBool()
                && session.value(QStringLiteral("id")).toString() == runningId) {
                m_running = session.toVariantMap();
                refreshRunning();
                break;
            }
        }
    }
    sessionResponse({}, false)(result);
}

CoreClient::Callback Backend::sessionResponse(const QString &pendingItemId, bool announce) {
    return [this, pendingItemId, announce](const CoreClient::Result &result) {
        if (!pendingItemId.isEmpty()) setItemPending(pendingItemId, false);
        if (result.ok) {
            m_refreshPending = true;
            if (announce) emit sessionWriteFinished(true, {}, result.opId);
            return;
        }
        fail(result.error.message);
        if (announce) emit sessionWriteFinished(false, result.error.message, result.opId);
        if (result.error.kind == CoreClient::ErrorKind::Conflict || !m_client.offline()) reload();
    };
}

void Backend::toggleTimer(const QString &id, bool running) {
    setItemPending(id, true);
    if (running) m_client.sessionStop({}, {}, {}, mutationResponse(id));
    else m_client.sessionStart(id, {}, mutationResponse(id));
}

void Backend::startSession(const QString &itemId, const QString &projectId) {
    if (!itemId.isEmpty()) setItemPending(itemId, true);
    m_client.sessionStart(itemId, itemId.isEmpty() ? projectId : QString(), sessionResponse(itemId));
}

void Backend::togglePrayer(const QString &prayer, bool marked) {
    setPrayerPending(prayer, true);
    const QDate day = m_selected.isValid() ? m_selected : m_today;
    const qint64 version = versionFor(QStringLiteral("%1:%2").arg(day.toString(Qt::ISODate), prayer));
    const QString pending = QStringLiteral("prayer:") + prayer;
    if (marked) m_client.prayerUnmark(prayer, m_selected, mutationResponse(pending), version);
    else m_client.prayerMark(prayer, m_selected, mutationResponse(pending), version);
}

void Backend::setSleepTime(const QString &time) {
    m_client.configSetSleepTime(time, readResponse([this](const CoreClient::Result &result) {
        if (const auto *config = std::get_if<Dto::Config>(&result.value)) applyConfig(*config);
    }, true));
}

void Backend::loadSettings() {
    // The list methods are served by the daemon only; the CLI fallback has
    // nothing to show, and an error banner for it would be noise.
    if (m_client.offline()) return;
    auto load = [this](const QString &method, QVariantList *target) {
        m_client.listEntities(method, readResponse([this, target](const CoreClient::Result &result) {
            if (const auto *json = std::get_if<Dto::Json>(&result.value)) {
                *target = json->value.toArray().toVariantList();
                emit settingsChanged();
                refreshRunning();
            }
        }));
    };
    load(QStringLiteral("projects.list"), &m_projects);
    load(QStringLiteral("epics.list"), &m_epics);
    load(QStringLiteral("tags.list"), &m_tags);
    load(QStringLiteral("metrics.list"), &m_metrics);
    loadGoals();
}

void Backend::saveEntity(const QString &kind, const QVariantMap &fields) {
    static const QSet<QString> allowed{QStringLiteral("project"), QStringLiteral("epic"), QStringLiteral("tag"),
                                       QStringLiteral("metric")};
    if (!allowed.contains(kind)) return;
    const bool editing = !fields.value(QStringLiteral("id")).toString().isEmpty();
    m_client.mutateEntity(kind + (editing ? QStringLiteral(".edit") : QStringLiteral(".add")),
                          QJsonObject::fromVariantMap(fields), entityResponse(kind));
}

void Backend::removeTag(const QString &id) {
    if (id.isEmpty()) return;
    m_client.mutateEntity(QStringLiteral("tag.remove"), QJsonObject{{QStringLiteral("id"), id}},
                          entityResponse(QStringLiteral("tag")));
}

CoreClient::Callback Backend::entityResponse(const QString &kind) {
    return [this, kind](const CoreClient::Result &result) {
        if (!result.ok) {
            fail(result.error.message);
            emit entitySaveFailed(kind, result.error.message);
            return;
        }
        QString id;
        if (const auto *mutation = std::get_if<Dto::MutationResult>(&result.value)) {
            const QJsonObject changed = mutation->json.value(QStringLiteral("changed")).toObject();
            const QJsonArray rows = changed.value(kind + QStringLiteral("s")).toArray();
            if (!rows.isEmpty()) id = rows.first().toObject().value(QStringLiteral("id")).toString();
        }
        clearError();
        loadSettings();
        // Project names and colours show on the Today rows.
        m_refreshPending = true;
        emit entitySaved(kind, id);
    };
}

void Backend::setSelectedStatsItem(const QString &id) {
    if (m_selectedStatsItem == id) return;
    m_selectedStatsItem = id;
    m_statsDaily.setStats(m_stats, todayDay(), m_selectedStatsItem);
    emit selectedStatsItemChanged();
}

ItemDraft *Backend::itemForId(const QString &id) {
    if (m_backlogOpen) {
        const QVariantMap row = backlogItem(id);
        if (!row.isEmpty()) {
            m_editItem->update(QJsonObject::fromVariantMap(row));
            emit editItemChanged();
            return m_editItem;
        }
    }
    bool found = false;
    for (const Dto::DayBlock &block : m_dayPlan.blocks) {
        for (const Dto::DayItem &item : block.items) {
            if (item.id != id) continue;
            m_editItem->update(item.json);
            found = true;
            break;
        }
        if (found) break;
    }
    if (!found) {
        const auto pending = std::find_if(m_pendingCreates.cbegin(), m_pendingCreates.cend(),
                                          [&](const PendingCreate &p) { return p.localId == id; });
        if (pending != m_pendingCreates.cend()) {
            QJsonObject row{{QStringLiteral("id"), pending->localId},
                            {QStringLiteral("title"), pending->title},
                            {QStringLiteral("block"), pending->block},
                            {QStringLiteral("repeat"), pending->repeat},
                            {QStringLiteral("project_id"), pending->projectId},
                            {QStringLiteral("planned_minutes"), pending->plannedMinutes}};
            QJsonArray days;
            for (const QString &day : pending->weekdays) days.append(day);
            row[QStringLiteral("weekdays")] = days;
            m_editItem->update(row);
            found = true;
        }
    }
    if (!found) {
        for (const Dto::StatsItem &item : m_routinesStats.items) {
            if (item.id != id) continue;
            m_editItem->update(item.json);
            found = true;
            break;
        }
    }
    if (!found) {
        for (const Dto::StatsItem &item : m_stats.items) {
            if (item.id != id) continue;
            m_editItem->update(item.json);
            found = true;
            break;
        }
    }
    if (!found) m_editItem->clear();
    emit editItemChanged();
    return m_editItem;
}

ItemDraft *Backend::itemFromParse(const QString &fallbackTitle) {
    if (fallbackTitle.isEmpty() || !m_parse.title.isEmpty()) {
        m_editItem->update(m_parse);
    } else {
        QJsonObject row = m_parse.json;
        row[QStringLiteral("title")] = fallbackTitle;
        m_editItem->update(row);
    }
    emit editItemChanged();
    return m_editItem;
}

QString Backend::hijriHeader(const QString &isoDate) const {
    const QDate date = QDate::fromString(isoDate, Qt::ISODate);
    if (!date.isValid()) return {};
    const QCalendar calendar(QCalendar::System::IslamicCivil);
    const auto parts = calendar.partsFromDate(date);
    static const QStringList months{tr("Muharram"), tr("Safar"), tr("Rabi al-Awwal"), tr("Rabi al-Thani"), tr("Jumada al-Awwal"), tr("Jumada al-Thani"), tr("Rajab"), tr("Sha'ban"), tr("Ramadan"), tr("Shawwal"), tr("Dhu al-Qadah"), tr("Dhu al-Hijjah")};
    const QString month = parts.month >= 1 && parts.month <= months.size() ? months.at(parts.month - 1) : QString();
    return tr("%1 %2 %3").arg(QLocale::c().dayName(date.dayOfWeek(), QLocale::LongFormat), QString::number(parts.day), month);
}

QString Backend::gregorianHeader(const QString &isoDate) const {
    const QDate date = QDate::fromString(isoDate, Qt::ISODate);
    return date.isValid() ? QLocale::c().toString(date, "MMMM d, yyyy") : QString();
}
