#include "backend.h"

#include <QCalendar>
#include <QDateTime>
#include <QJsonArray>
#include <QJsonObject>
#include <QLocale>
#include <algorithm>

namespace {
bool removeFromBlocks(QVariantList &blocks, const QString &id) {
    bool removed = false;
    for (int b = 0; b < blocks.size(); ++b) {
        QVariantMap block = blocks.at(b).toMap();
        QVariantList items = block.value("items").toList();
        for (int i = 0; i < items.size();) {
            if (items.at(i).toMap().value("id").toString() == id) {
                items.removeAt(i);
                removed = true;
            } else {
                ++i;
            }
        }
        block["items"] = items;
        blocks[b] = block;
    }
    return removed;
}

void sortBlock(QVariantList &items) {
    std::sort(items.begin(), items.end(), [](const QVariant &a, const QVariant &b) {
        const QVariantMap left = a.toMap();
        const QVariantMap right = b.toMap();
        const int lp = left.value("position", 0).toInt();
        const int rp = right.value("position", 0).toInt();
        if (lp != rp) return lp < rp;
        return left.value("id").toString() < right.value("id").toString();
    });
}
} // namespace

Backend::Backend(QObject *parent, const QString &program, int timeoutMs, const QString &socketPath)
    : QObject(parent), m_client(this, program, timeoutMs, socketPath) {
    connect(&m_client, &CoreClient::busyChanged, this, &Backend::changed);
    connect(&m_client, &CoreClient::offlineChanged, this, [this](bool) { emit changed(); });
    connect(&m_client, &CoreClient::reconnected, this, [this] { reload(); });
    connect(&m_client, &CoreClient::changed, this, &Backend::reloadFromNotification);
    connect(&m_client, &CoreClient::requestFinished, this, [this](const QStringList &args, bool success) {
        emit requestFinished(args, success);
        if (!m_client.busy() && m_refreshPending) {
            m_refreshPending = false;
            reload();
        }
    });
}

Backend::~Backend() = default;

CoreClient::Callback Backend::response(std::function<void(const QJsonObject &)> success, bool reloadAfter) {
    return [this, success = std::move(success), reloadAfter](const CoreClient::Result &result) {
        if (!result.ok) fail(result.error.message);
        else {
            if (success) success(Dto::jsonObject(result.value));
            if (reloadAfter) m_refreshPending = true;
        }
        emit changed();
    };
}

CoreClient::Callback Backend::mutationResponse(const QString &pendingId, const QString &pendingOpId) {
    return [this, pendingId, pendingOpId](const CoreClient::Result &result) {
        if (result.ok) {
            if (const auto *mutation = std::get_if<Dto::MutationResult>(&result.value)) {
                applyMutation(*mutation);
            } else {
                // Legacy CLI fallback: it does not return canonical rows, so
                // refetch once the queue drains.
                if (!pendingOpId.isEmpty()) removePendingCreate(pendingOpId);
                if (pendingId.startsWith("prayer:")) setPrayerPending(pendingId.mid(7), false);
                else if (!pendingId.isEmpty()) setItemPending(pendingId, false);
                m_refreshPending = true;
            }
        } else {
            if (!pendingOpId.isEmpty()) removePendingCreate(pendingOpId);
            if (pendingId.startsWith("prayer:")) setPrayerPending(pendingId.mid(7), false);
            else if (!pendingId.isEmpty()) setItemPending(pendingId, false);
            fail(result.error.message);
            if (result.error.kind == CoreClient::ErrorKind::Conflict) {
                // The entity changed underneath us; refetch the affected view
                // and keep the server's human message in the error banner.
                reload();
            } else if (!m_client.offline()) {
                reload();
            }
        }
        emit changed();
    };
}

void Backend::fail(const QString &message) { m_error = message; }
void Backend::dismissError() { m_error.clear(); emit changed(); }

QVariantMap Backend::normalizedObject(const QJsonObject &object) { return object.toVariantMap(); }

bool Backend::setJsonModel(QVariantMap &target, QByteArray &previous, const QJsonObject &object) {
    const QByteArray json = QJsonDocument(object).toJson(QJsonDocument::Compact);
    if (json == previous) return false;
    previous = json;
    target = normalizedObject(object);
    return true;
}

void Backend::reload() {
    m_client.day(m_selected, response([this](const QJsonObject &object) {
        const bool modelChanged = setJsonModel(m_dayPlan, m_dayJson, object);
        m_dayPlanFetchedAt = QDateTime::currentMSecsSinceEpoch();
        const QDate cliDay = QDate::fromString(object.value("day").toString(), Qt::ISODate);
        if (cliDay.isValid()) {
            m_selected = cliDay;
            if (object.value("is_today").toBool()) m_today = cliDay;
        }
        for (const QVariant &entry : m_dayPlan.value("blocks").toList()) {
            for (const QVariant &item : entry.toMap().value("items").toList()) {
                const QVariantMap map = item.toMap();
                if (map.contains("version")) m_versions.insert(map.value("id").toString(), map.value("version").toLongLong());
            }
        }
        applyPendingFlags();
        if (modelChanged) { ++m_dayRevision; emit changed(); }
    }));
    m_client.statsDays(14, response([this](const QJsonObject &object) {
        if (setJsonModel(m_routinesStats, m_routinesJson, object)) emit changed();
    }));
    m_client.configShow(response([this](const QJsonObject &object) {
        if (setJsonModel(m_config, m_configJson, object)) emit changed();
    }));
}

void Backend::reloadFromNotification(qint64 revision) {
    if (revision <= m_lastNotificationRevision) return;
    m_lastNotificationRevision = revision;
    reload();
}

void Backend::loadStats(int days) {
    if (days != 7 && days != 30) days = 30;
    m_client.statsDays(days, response([this](const QJsonObject &object) {
        if (setJsonModel(m_stats, m_statsJson, object)) emit changed();
    }));
}

void Backend::loadStatsRange(const QString &from, const QString &to) {
    const QDate first = QDate::fromString(from, Qt::ISODate);
    const QDate last = QDate::fromString(to, Qt::ISODate);
    if (!first.isValid() || !last.isValid() || first > last || first.daysTo(last) >= 366) {
        fail(tr("Invalid statistics date range.")); emit changed(); return;
    }
    m_client.stats(first, last, response([this](const QJsonObject &object) {
        if (setJsonModel(m_stats, m_statsJson, object)) emit changed();
    }));
}

void Backend::moveDay(int amount) { if (!m_selected.isValid()) m_selected = m_today; m_selected = m_selected.addDays(amount); reload(); }
void Backend::selectDay(const QString &day) { const QDate parsed = QDate::fromString(day, Qt::ISODate); if (parsed.isValid() && parsed != m_selected) { m_selected = parsed; reload(); } }
void Backend::jumpToday() { m_selected = m_today; reload(); }
void Backend::parseQuick(const QString &text) {
    if (text.trimmed().isEmpty()) { m_parseDraft.clear(); emit changed(); return; }
    m_client.parse(text, response([this](const QJsonObject &object) {
        m_parseDraft = normalizedObject(object);
        emit changed();
    }));
}

void Backend::previewItem(const QVariantMap &item) {
    const QString repeat = item.value("repeat", "none").toString();
    QStringList weekdays = item.value("weekdays").toStringList();
    if (repeat == "weekly" && weekdays.isEmpty()) {
        m_preview.clear();
        emit previewChanged();
        return;
    }
    CoreClient::ItemFields fields;
    fields.block = item.value("block", "fajr").toString();
    if (fields.block.isEmpty() || fields.block == "sleep") fields.block = "fajr";
    fields.repeat = repeat;
    fields.weekdays = weekdays;
    fields.measure = item.value("measure", "check").toString();
    const int target = item.value("target").toInt();
    fields.hasTarget = fields.measure != "check" && target > 0;
    fields.target = target;
    fields.date = m_selected.isValid() ? m_selected : m_today;
    m_client.preview(fields, [this](const CoreClient::Result &result) {
        m_preview = result.ok ? normalizedObject(Dto::jsonObject(result.value)) : QVariantMap{};
        emit previewChanged();
    });
}

void Backend::addQuick(const QString &text, const QString &block) {
    CoreClient::ItemFields fields;
    fields.title = text;
    fields.block = block.isEmpty() ? QStringLiteral("fajr") : block;
    fields.quick = true;
    fields.opId = CoreClient::makeOpId();
    if (m_selected.isValid() && m_selected != m_today) fields.date = m_selected;
    const QString pendingId = QStringLiteral("pending-%1").arg(fields.opId);
    const QVariantMap pending{{"id", pendingId}, {"title", text}, {"block", fields.block}, {"position", 9999},
                              {"repeat", "none"}, {"measure", "check"}, {"state", "open"}, {"pending", true}};
    insertPendingItem(pending);
    m_pendingCreates.insert(fields.opId, pending);
    m_client.itemAdd(fields, mutationResponse(pendingId, fields.opId));
}

void Backend::insertPendingItem(const QVariantMap &source) {
    QVariantMap item = source;
    const QString target = item.value("block", "fajr").toString();
    QVariantList blocks = m_dayPlan.value("blocks").toList();
    for (int b = 0; b < blocks.size(); ++b) {
        QVariantMap block = blocks[b].toMap();
        if (block.value("start").toString() != target) continue;
        QVariantList items = block.value("items").toList();
        item["position"] = items.size();
        items.append(item);
        block["items"] = items;
        blocks[b] = block;
        m_dayPlan["blocks"] = blocks;
        m_pendingItems.insert(item.value("id").toString());
        m_dayJson.clear();
        ++m_dayRevision;
        emit changed();
        return;
    }
}

void Backend::removePendingCreate(const QString &opId) {
    const auto it = m_pendingCreates.find(opId);
    if (it == m_pendingCreates.end()) return;
    const QString id = it.value().value("id").toString();
    m_pendingCreates.erase(it);
    m_pendingItems.remove(id);
    if (removeItemFromModel(id)) {
        ++m_dayRevision;
        emit changed();
    }
}

qint64 Backend::versionFor(const QString &id) const {
    const auto it = m_versions.constFind(id);
    return it == m_versions.constEnd() ? -1 : it.value();
}

bool Backend::removeItemFromModel(const QString &id) {
    QVariantList blocks = m_dayPlan.value("blocks").toList();
    if (!removeFromBlocks(blocks, id)) return false;
    m_dayPlan["blocks"] = blocks;
    m_dayJson.clear();
    return true;
}

void Backend::setItemPending(const QString &id, bool pending) {
    if (pending) m_pendingItems.insert(id);
    else m_pendingItems.remove(id);
    QVariantList blocks = m_dayPlan.value("blocks").toList();
    bool found = false;
    for (int b = 0; b < blocks.size(); ++b) {
        QVariantMap block = blocks[b].toMap();
        QVariantList items = block.value("items").toList();
        for (int i = 0; i < items.size(); ++i) {
            QVariantMap item = items[i].toMap();
            if (item.value("id").toString() != id) continue;
            if (pending) item["pending"] = true;
            else item.remove("pending");
            items[i] = item;
            found = true;
        }
        block["items"] = items;
        blocks[b] = block;
    }
    if (found) {
        m_dayPlan["blocks"] = blocks;
        m_dayJson.clear();
        ++m_dayRevision;
        emit changed();
    }
}

void Backend::setPrayerPending(const QString &prayer, bool pending) {
    if (pending) m_pendingPrayers.insert(prayer);
    else m_pendingPrayers.remove(prayer);
    QVariantList prayers = m_dayPlan.value("prayers").toList();
    for (int i = 0; i < prayers.size(); ++i) {
        QVariantMap item = prayers[i].toMap();
        if (item.value("id").toString() != prayer) continue;
        if (pending) item["pending"] = true;
        else item.remove("pending");
        prayers[i] = item;
        m_dayPlan["prayers"] = prayers;
        m_dayJson.clear();
        ++m_dayRevision;
        emit changed();
        return;
    }
}

void Backend::applyMutation(const Dto::MutationResult &mutation) {
    removePendingCreate(mutation.opId);
    QVariantList blocks = m_dayPlan.value("blocks").toList();
    for (const QString &id : mutation.deleted) {
        removeFromBlocks(blocks, id);
        m_pendingItems.remove(id);
        m_versions.remove(id);
    }
    for (const Dto::DayItem &row : mutation.items) {
        const QVariantMap item = normalizedObject(row.json);
        const QString id = item.value("id").toString();
        if (item.contains("version")) m_versions.insert(id, item.value("version").toLongLong());
        m_pendingItems.remove(id);
        removeFromBlocks(blocks, id);
        const QString block = item.value("block").toString();
        for (int b = 0; b < blocks.size(); ++b) {
            QVariantMap target = blocks[b].toMap();
            if (target.value("start").toString() != block) continue;
            QVariantList items = target.value("items").toList();
            items.append(item);
            sortBlock(items);
            target["items"] = items;
            blocks[b] = target;
            break;
        }
    }
    m_dayPlan["blocks"] = blocks;
    for (const Dto::PrayerMark &mark : mutation.marks) {
        if (mark.json.contains("version")) m_versions.insert(mark.id, mark.json.value("version").toVariant().toLongLong());
        m_pendingPrayers.remove(mark.prayer);
        QVariantList prayers = m_dayPlan.value("prayers").toList();
        for (int i = 0; i < prayers.size(); ++i) {
            QVariantMap item = prayers[i].toMap();
            if (item.value("id").toString() != mark.prayer) continue;
            item["marked"] = mark.json.value("deleted_at").isNull() || !mark.json.contains("deleted_at");
            item.remove("pending");
            prayers[i] = item;
            m_dayPlan["prayers"] = prayers;
            break;
        }
    }
    m_dayPlanFetchedAt = QDateTime::currentMSecsSinceEpoch();
    m_dayJson.clear();
    ++m_dayRevision;
}

void Backend::applyPendingFlags() {
    QVariantList blocks = m_dayPlan.value("blocks").toList();
    bool changedModel = false;
    for (auto it = m_pendingCreates.cbegin(); it != m_pendingCreates.cend(); ++it) {
        const QVariantMap pending = it.value();
        const QString id = pending.value("id").toString();
        bool present = false;
        for (const QVariant &blockValue : std::as_const(blocks))
            for (const QVariant &row : blockValue.toMap().value("items").toList())
                if (row.toMap().value("id").toString() == id) present = true;
        if (present) continue;
        const QString target = pending.value("block").toString();
        for (int b = 0; b < blocks.size(); ++b) {
            QVariantMap block = blocks[b].toMap();
            if (block.value("start").toString() != target) continue;
            QVariantList items = block.value("items").toList();
            items.append(pending);
            block["items"] = items;
            blocks[b] = block;
            changedModel = true;
            break;
        }
    }
    for (int b = 0; b < blocks.size(); ++b) {
        QVariantMap block = blocks[b].toMap();
        QVariantList items = block.value("items").toList();
        for (int i = 0; i < items.size(); ++i) {
            QVariantMap item = items[i].toMap();
            const QString id = item.value("id").toString();
            const bool pending = m_pendingItems.contains(id);
            if (pending == item.contains("pending")) continue;
            if (pending) item["pending"] = true;
            else item.remove("pending");
            items[i] = item;
            changedModel = true;
        }
        block["items"] = items;
        blocks[b] = block;
    }
    if (changedModel) {
        m_dayPlan["blocks"] = blocks;
        m_dayJson.clear();
        ++m_dayRevision;
        emit changed();
    }
}

void Backend::saveItem(const QVariantMap &item) {
    const QString id = item.value("id").toString();
    CoreClient::ItemFields fields;
    fields.title = item.value("title").toString();
    fields.block = item.value("block", "fajr").toString();
    fields.repeat = item.value("repeat", "none").toString();
    fields.weekdays = item.value("weekdays").toStringList();
    fields.measure = item.value("measure", "check").toString();
    const int target = item.value("target").toInt();
    fields.hasTarget = fields.measure != "check" && target > 0;
    fields.target = target;
    if (id.isEmpty()) {
        if (fields.repeat == "none" && m_selected != m_today) fields.date = m_selected;
        fields.opId = CoreClient::makeOpId();
        const QString pendingId = QStringLiteral("pending-%1").arg(fields.opId);
        const QVariantMap pending{{"id", pendingId}, {"title", fields.title}, {"block", fields.block}, {"position", 9999},
                                  {"repeat", fields.repeat}, {"measure", fields.measure}, {"state", "open"}, {"pending", true}};
        insertPendingItem(pending);
        m_pendingCreates.insert(fields.opId, pending);
        m_client.itemAdd(fields, mutationResponse(pendingId, fields.opId));
    } else {
        setItemPending(id, true);
        m_client.itemEdit(id, fields, mutationResponse(id), versionFor(id));
    }
}

void Backend::removeItem(const QString &id) {
    setItemPending(id, true);
    m_client.itemRemove(id, mutationResponse(id), versionFor(id));
}
int Backend::legacyMoveIndex(const QString &id, const QString &block, const QString &afterId, const QString &beforeId) const {
    // The socket protocol resolves anchors in the write transaction; the
    // legacy CLI only accepts an index, so derive it from the rendered model.
    QVariantList peers;
    for (const QVariant &entry : m_dayPlan.value("blocks").toList()) {
        const QVariantMap target = entry.toMap();
        if (target.value("start").toString() != block) continue;
        for (const QVariant &row : target.value("items").toList()) {
            const QVariantMap map = row.toMap();
            if (map.value("id").toString() != id) peers.append(map);
        }
        break;
    }
    for (int i = 0; i < peers.size(); ++i) {
        const QString peer = peers.at(i).toMap().value("id").toString();
        if (!afterId.isEmpty() && peer == afterId) return i + 1;
        if (!beforeId.isEmpty() && peer == beforeId) return i;
    }
    return 0;
}

void Backend::moveItem(const QString &id, const QString &block, const QString &afterId, const QString &beforeId) {
    setItemPending(id, true);
    const int index = legacyMoveIndex(id, block, afterId, beforeId);
    m_client.itemMove(id, block, index, mutationResponse(id), versionFor(id), afterId, beforeId);
}
void Backend::toggleDone(const QString &id, const QString &state) {
    setItemPending(id, true);
    if (state == "done") m_client.itemUndo(id, m_selected, mutationResponse(id));
    else m_client.itemDone(id, m_selected, mutationResponse(id));
}
void Backend::adjustMinutes(const QString &id, int amount) {
    setItemPending(id, true);
    m_client.itemLog(id, amount, true, m_selected != m_today ? m_selected : QDate(), mutationResponse(id));
}
void Backend::adjustCount(const QString &id, int amount) {
    setItemPending(id, true);
    m_client.itemLog(id, amount, false, m_selected != m_today ? m_selected : QDate(), mutationResponse(id));
}
void Backend::toggleTimer(const QString &id, bool running) {
    setItemPending(id, true);
    if (running) m_client.timerStop(id, mutationResponse(id));
    else m_client.timerStart(id, mutationResponse(id));
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
    m_client.configSetSleepTime(time, response({}, true));
}

QString Backend::hijriHeader(const QString &isoDate) const {
    const QDate date = QDate::fromString(isoDate, Qt::ISODate); if (!date.isValid()) return {};
    const QCalendar calendar(QCalendar::System::IslamicCivil); const auto parts = calendar.partsFromDate(date);
    static const QStringList months{tr("Muharram"), tr("Safar"), tr("Rabi al-Awwal"), tr("Rabi al-Thani"), tr("Jumada al-Awwal"), tr("Jumada al-Thani"), tr("Rajab"), tr("Sha'ban"), tr("Ramadan"), tr("Shawwal"), tr("Dhu al-Qadah"), tr("Dhu al-Hijjah")};
    const QString month = parts.month >= 1 && parts.month <= months.size() ? months.at(parts.month - 1) : QString();
    return tr("%1 %2 %3").arg(QLocale::c().dayName(date.dayOfWeek(), QLocale::LongFormat), QString::number(parts.day), month);
}

QString Backend::gregorianHeader(const QString &isoDate) const { const QDate date = QDate::fromString(isoDate, Qt::ISODate); return date.isValid() ? QLocale::c().toString(date, "MMMM d, yyyy") : QString(); }
