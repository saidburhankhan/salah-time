#include "daytimelinemodel.h"

DayTimelineModel::DayTimelineModel(QObject *parent) : TypedListModel(parent) {
    setRoleNames({
        {RowKindRole, "rowKind"}, {RowKeyRole, "rowKey"}, {BlockRole, "block"},
        {NameRole, "name"}, {TimeRole, "time"}, {MarkedRole, "marked"},
        {CurrentRole, "current"}, {ItemIdRole, "itemId"}, {TitleRole, "title"},
        {StateRole, "state"}, {MeasureRole, "measure"}, {RepeatRole, "repeat"},
        {DetailTextRole, "detailText"}, {ScheduleTextRole, "scheduleText"},
        {NextOccurrenceRole, "nextOccurrence"}, {CarriedRole, "carried"},
        {PendingRole, "pending"}, {TimerRunningRole, "timerRunning"},
        {TimerStartedAtRole, "timerStartedAt"}, {TimerMinutesRole, "timerMinutes"},
        {TimerProjectedRole, "timerProjected"}, {CountRole, "count"},
        {TargetRole, "target"}, {WeekdaysRole, "weekdays"}, {VersionRole, "version"},
        {PositionRole, "position"}, {ProjectColorRole, "projectColor"},
        {PlannedMinutesRole, "plannedMinutes"}, {ActualMinutesRole, "actualMinutes"},
        {SessionsRole, "sessions"}, {CapacityMinutesRole, "capacityMinutes"},
        {BlockPlannedMinutesRole, "blockPlannedMinutes"}, {BlockActualMinutesRole, "blockActualMinutes"}
    });
}

QString DayTimelineModel::kindName(Kind kind) {
    switch (kind) {
    case Kind::Header: return QStringLiteral("header");
    case Kind::Item: return QStringLiteral("item");
    case Kind::Add: return QStringLiteral("add");
    case Kind::Marker: return QStringLiteral("marker");
    }
    return {};
}

QVariantList DayTimelineModel::values(const Row &row) {
    return {kindName(row.kind), row.key, row.block, row.name, row.time,
            row.marked, row.current, row.itemId, row.title, row.state,
            row.measure, row.repeat, row.detailText, row.scheduleText,
            row.nextOccurrence, row.carried, row.pending, row.timerRunning,
            row.timerStartedAt, row.timerMinutes, row.timerProjected, row.count,
            row.target, row.weekdays, row.version, row.position, row.projectColor,
            row.plannedMinutes, row.actualMinutes, row.sessions, row.capacityMinutes,
            row.blockPlannedMinutes, row.blockActualMinutes};
}

void DayTimelineModel::setRows(const QVector<Row> &rows) {
    QStringList keys;
    QVector<QVariantList> data;
    keys.reserve(rows.size());
    data.reserve(rows.size());
    for (const Row &row : rows) {
        keys.append(row.key);
        data.append(values(row));
    }
    setRows(keys, data);
}

QString DayTimelineModel::kindAt(int row) const {
    return valueAt(row, RowKindRole).toString();
}

QString DayTimelineModel::keyAt(int row) const {
    return valueAt(row, RowKeyRole).toString();
}

QString DayTimelineModel::blockAt(int row) const {
    return valueAt(row, BlockRole).toString();
}

QString DayTimelineModel::itemIdAt(int row) const {
    return valueAt(row, ItemIdRole).toString();
}

QString DayTimelineModel::titleAt(int row) const {
    return valueAt(row, TitleRole).toString();
}

QString DayTimelineModel::detailTextAt(int row) const {
    return valueAt(row, DetailTextRole).toString();
}

QString DayTimelineModel::stateAt(int row) const {
    return valueAt(row, StateRole).toString();
}

int DayTimelineModel::indexOfItem(const QString &itemId) const {
    return indexOfKey(QStringLiteral("item:") + itemId);
}
