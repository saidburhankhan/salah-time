#include "statsitemsmodel.h"

#include <QJsonArray>
#include <algorithm>

#include "statsseriesmodel.h"

StatsItemsModel::StatsItemsModel(QObject *parent) : TypedListModel(parent) {
    setRoleNames({{ItemIdRole, "itemId"}, {TitleRole, "title"}, {BlockRole, "block"},
                  {RepeatRole, "repeat"}, {MeasureRole, "measure"},
                  {ScheduleTextRole, "scheduleText"}, {NextOccurrenceRole, "nextOccurrence"},
                  {PlannedDaysRole, "plannedDays"}, {DoneDaysRole, "doneDays"},
                  {MinutesRole, "minutes"}, {CountRole, "count"}, {TargetRole, "target"},
                  {WeekdaysRole, "weekdays"}, {SeriesRole, "series"}});
}

StatsItemsModel::~StatsItemsModel() {
    qDeleteAll(m_series);
    m_series.clear();
}

QVariantList StatsItemsModel::values(const Row &row) {
    return {row.itemId, row.title, row.block, row.repeat, row.measure, row.scheduleText,
            row.nextOccurrence, row.plannedDays, row.doneDays, row.minutes, row.count,
            row.target, row.weekdays, QVariant::fromValue<QObject *>(row.series)};
}

StatsSeriesModel *StatsItemsModel::seriesFor(const QString &id) {
    auto it = m_series.find(id);
    if (it != m_series.end()) return it.value();
    auto *model = new StatsSeriesModel(this);
    m_series.insert(id, model);
    return model;
}

void StatsItemsModel::setItems(const std::vector<Dto::StatsItem> &items, Sort sort) {
    std::vector<Dto::StatsItem> ordered = items;
    if (sort == Sort::MinutesDescending) {
        std::stable_sort(ordered.begin(), ordered.end(), [](const Dto::StatsItem &a, const Dto::StatsItem &b) {
            return a.json.value("minutes").toInt() > b.json.value("minutes").toInt();
        });
    }

    QStringList liveIds;
    QVector<Row> rows;
    rows.reserve(int(ordered.size()));
    for (const Dto::StatsItem &item : ordered) {
        Row row;
        row.itemId = item.id;
        row.title = item.title;
        row.block = item.block;
        row.repeat = item.repeat;
        row.measure = item.measure;
        row.scheduleText = item.schedule_text;
        row.nextOccurrence = item.next_occurrence;
        row.plannedDays = item.json.value("planned_days").toInt();
        row.doneDays = item.json.value("done_days").toInt();
        row.minutes = item.json.value("minutes").toInt();
        row.count = item.json.value("count").toInt();
        row.target = item.json.value("target").toInt();
        const QJsonArray weekdays = item.json.value("weekdays").toArray();
        for (const QJsonValue &day : weekdays) row.weekdays.append(day.toString());
        row.series = seriesFor(item.id);
        QVector<StatsSeriesModel::Row> series;
        series.reserve(int(item.series.size()));
        for (const Dto::StatsSeries &day : item.series) {
            StatsSeriesModel::Row seriesRow;
            seriesRow.day = day.day;
            seriesRow.planned = day.json.value("planned").toBool();
            seriesRow.done = day.json.value("done").toBool();
            seriesRow.minutes = day.json.value("minutes").toInt();
            seriesRow.count = day.json.value("count").toInt();
            series.append(seriesRow);
        }
        row.series->setRows(series);
        liveIds.append(row.itemId);
        rows.append(row);
    }
    setRows(rows);
    pruneSeries(liveIds);
}

void StatsItemsModel::setRows(const QVector<Row> &rows) {
    QStringList keys;
    QVector<QVariantList> data;
    keys.reserve(rows.size());
    data.reserve(rows.size());
    for (const Row &row : rows) {
        keys.append(row.itemId);
        data.append(values(row));
    }
    setRows(keys, data);
}

void StatsItemsModel::pruneSeries(const QStringList &liveIds) {
    const QStringList known = m_series.keys();
    for (const QString &id : known) {
        if (liveIds.contains(id)) continue;
        StatsSeriesModel *orphan = m_series.take(id);
        orphan->deleteLater();
    }
}
