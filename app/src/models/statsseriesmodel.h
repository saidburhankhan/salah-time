#pragma once

#include "typedlistmodel.h"

// Per-day series for one statistics item or routine. Each parent row owns one
// child model, so a delegate can iterate the series without flattening or
// re-joining daily rows in QML.
class StatsSeriesModel : public TypedListModel {
    Q_OBJECT
public:
    enum Role { DayRole = Qt::UserRole + 1, PlannedRole, DoneRole, MinutesRole, CountRole };

    struct Row {
        QString day;
        bool planned = false;
        bool done = false;
        int minutes = 0;
        int count = 0;
    };

    explicit StatsSeriesModel(QObject *parent = nullptr);

    using TypedListModel::setRows;
    void setRows(const QVector<Row> &rows);

private:
    static QVariantList values(const Row &row);
};
