#include "viewstate.h"

#include <QJsonArray>

NowState::NowState(QObject *parent) : QObject(parent) {}

void NowState::update(const Dto::DayPlan &plan, qint64 fetchedAt) {
    m_day = plan.day;
    m_sunrise = plan.json.value(QStringLiteral("sunrise")).toString();
    m_nightStart = plan.json.value(QStringLiteral("night_start")).toString();
    m_sleepTime = plan.sleep.time;
    m_sleepPassed = plan.sleep.json.value(QStringLiteral("passed")).toBool();
    m_block.clear();
    m_sleeping = false;
    m_nextId.clear();
    m_nextName.clear();
    m_nextTime.clear();
    m_nextAt.clear();
    m_nextRemainingSeconds = 0;
    if (plan.now) {
        m_block = plan.now->block;
        m_sleeping = plan.now->json.value(QStringLiteral("sleeping")).toBool();
        const QJsonObject next = plan.now->json.value(QStringLiteral("next")).toObject();
        m_nextId = next.value(QStringLiteral("id")).toString();
        m_nextName = next.value(QStringLiteral("name")).toString();
        m_nextTime = next.value(QStringLiteral("time")).toString();
        m_nextAt = next.value(QStringLiteral("at")).toString();
        m_nextRemainingSeconds = next.value(QStringLiteral("remaining_seconds")).toVariant().toLongLong();
    }
    m_fetchedAt = fetchedAt;
    emit changed();
}

StatsTotals::StatsTotals(QObject *parent) : QObject(parent) {}

void StatsTotals::update(const Dto::Stats &stats) {
    const QJsonObject totals = stats.json.value(QStringLiteral("totals")).toObject();
    set(totals.value(QStringLiteral("planned")).toInt(),
        totals.value(QStringLiteral("done")).toInt(),
        totals.value(QStringLiteral("minutes")).toInt(),
        totals.value(QStringLiteral("count")).toInt(),
        totals.value(QStringLiteral("prayers")).toInt(),
        totals.value(QStringLiteral("active_days")).toInt(),
        stats.json.value(QStringLiteral("prayer_slots")).toInt(),
        stats.json.value(QStringLiteral("days")).toInt());
}

void StatsTotals::set(int planned, int done, int minutes, int count, int prayers,
                      int activeDays, int prayerSlots, int days) {
    if (m_planned == planned && m_done == done && m_minutes == minutes && m_count == count &&
        m_prayers == prayers && m_activeDays == activeDays && m_prayerSlots == prayerSlots &&
        m_days == days)
        return;
    m_planned = planned;
    m_done = done;
    m_minutes = minutes;
    m_count = count;
    m_prayers = prayers;
    m_activeDays = activeDays;
    m_prayerSlots = prayerSlots;
    m_days = days;
    emit changed();
}

Config::Config(QObject *parent) : QObject(parent) {}

void Config::update(const Dto::Config &config) {
    const QJsonObject json = config.json;
    m_location = config.location;
    m_locationName = json.value(QStringLiteral("location_name")).toString();
    if (m_locationName.isEmpty()) m_locationName = json.value(QStringLiteral("name")).toString();
    if (m_locationName.isEmpty()) m_locationName = m_location;
    m_method = config.method;
    m_methodName = json.value(QStringLiteral("method_name")).toString();
    if (m_methodName.isEmpty()) m_methodName = m_method;
    m_sleepTime = config.sleepTime;
    m_asr = config.asr;
    m_autoLocation = json.value(QStringLiteral("auto_location")).toBool();
    m_latitude = json.value(QStringLiteral("latitude")).toDouble();
    m_longitude = json.value(QStringLiteral("longitude")).toDouble();
    m_timezoneOffset = json.value(QStringLiteral("timezone_offset")).toDouble();
    m_lastLocationProvider = json.value(QStringLiteral("last_location_provider")).toString();
    m_lastLocationSync = json.value(QStringLiteral("last_location_sync")).toString();
    emit changed();
}

ParseDraftState::ParseDraftState(QObject *parent) : QObject(parent) {}

void ParseDraftState::update(const Dto::ParseDraft &draft) {
    m_title = draft.title;
    m_block = draft.json.value(QStringLiteral("block")).toString();
    m_repeat = draft.repeat;
    m_plannedMinutes = draft.plannedMinutes;
    m_hasPlannedMinutes = draft.hasPlannedMinutes;
    m_weekdays.clear();
    for (const QJsonValue &day : draft.json.value(QStringLiteral("weekdays")).toArray())
        m_weekdays.append(day.toString());
    emit changed();
}

void ParseDraftState::clear() {
    m_title.clear();
    m_block.clear();
    m_repeat.clear();
    m_plannedMinutes = 0;
    m_hasPlannedMinutes = false;
    m_weekdays.clear();
    emit changed();
}

PreviewState::PreviewState(QObject *parent) : QObject(parent) {}

void PreviewState::update(const Dto::Preview &preview) {
    m_nextOccurrence = preview.next_occurrence;
    m_occurrencesThisMonth = preview.occurrences_this_month;
    m_scheduleText = preview.schedule_text;
    m_valid = true;
    emit changed();
}

void PreviewState::clear() {
    m_nextOccurrence.clear();
    m_occurrencesThisMonth = 0;
    m_scheduleText.clear();
    m_valid = false;
    emit changed();
}

ItemDraft::ItemDraft(QObject *parent) : QObject(parent) {}

void ItemDraft::update(const QJsonObject &row) {
    m_id = row.value(QStringLiteral("id")).toString();
    m_title = row.value(QStringLiteral("title")).toString();
    m_block = row.value(QStringLiteral("block")).toString();
    if (m_block.isEmpty()) m_block = QStringLiteral("fajr");
    m_repeat = row.value(QStringLiteral("repeat")).toString();
    if (m_repeat.isEmpty()) m_repeat = QStringLiteral("none");
    m_weekdays.clear();
    for (const QJsonValue &day : row.value(QStringLiteral("weekdays")).toArray())
        m_weekdays.append(day.toString());
    m_measure = row.value(QStringLiteral("measure")).toString();
    if (m_measure.isEmpty()) m_measure = QStringLiteral("check");
    m_target = row.value(QStringLiteral("target")).toInt();
    // A day row carries `project: {id, name, color}`; a pending create or an
    // item view carries `project_id`.
    m_projectId = row.value(QStringLiteral("project_id")).toString();
    if (m_projectId.isEmpty())
        m_projectId = row.value(QStringLiteral("project")).toObject().value(QStringLiteral("id")).toString();
    m_metricIds = row.value(QStringLiteral("metric_ids")).toVariant();
    m_plannedMinutes = row.value(QStringLiteral("planned_minutes")).toInt();
    emit changed();
}

void ItemDraft::update(const Dto::ParseDraft &draft) {
    update(draft.json);
}

void ItemDraft::clear() {
    m_id.clear();
    m_title.clear();
    m_block = QStringLiteral("fajr");
    m_repeat = QStringLiteral("none");
    m_weekdays.clear();
    m_measure = QStringLiteral("check");
    m_target = 0;
    m_projectId.clear();
    m_metricIds.clear();
    m_plannedMinutes = 0;
    emit changed();
}
