#include "statsticksmodel.h"

StatsTicksModel::StatsTicksModel(QObject *parent) : TypedListModel(parent) {
    setRoleNames({{MinutesRole, "minutes"}, {LabelRole, "label"}});
}

QVariantList StatsTicksModel::values(const Row &row) {
    return {row.minutes, row.label};
}

void StatsTicksModel::setRows(const QVector<Row> &rows) {
    QStringList keys;
    QVector<QVariantList> data;
    keys.reserve(rows.size());
    data.reserve(rows.size());
    for (int i = 0; i < rows.size(); ++i) {
        keys.append(QString::number(i));
        data.append(values(rows.at(i)));
    }
    setRows(keys, data);
}

QString StatsTicksModel::labelAt(int row) const {
    return valueAt(row, LabelRole).toString();
}

int StatsTicksModel::minutesAt(int row) const {
    return valueAt(row, MinutesRole).toInt();
}

void StatsTicksModel::setTicks(const std::vector<Dto::StatsTick> &ticks) {
    QVector<Row> rows;
    rows.reserve(int(ticks.size()));
    for (const Dto::StatsTick &tick : ticks) rows.append({int(tick.minutes), tick.label});
    setRows(rows);
    const int last = rows.isEmpty() ? 0 : rows.last().minutes;
    if (m_lastMinutes != last) {
        m_lastMinutes = last;
        emit lastMinutesChanged();
    }
}
