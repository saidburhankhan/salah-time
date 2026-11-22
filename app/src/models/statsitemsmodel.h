#pragma once

#include <QHash>
#include <vector>

#include "typedlistmodel.h"
#include "../client/dto.h"

class StatsSeriesModel;

// One row per statistics item. The per-day series is a child model owned by the
// row, so QML binds `required property var series` and iterates required role
// properties rather than reaching into nested maps.
class StatsItemsModel : public TypedListModel {
    Q_OBJECT
public:
    enum Role {
        ItemIdRole = Qt::UserRole + 1,
        TitleRole,
        BlockRole,
        RepeatRole,
        MeasureRole,
        ScheduleTextRole,
        NextOccurrenceRole,
        PlannedDaysRole,
        DoneDaysRole,
        MinutesRole,
        CountRole,
        TargetRole,
        WeekdaysRole,
        SeriesRole
    };

    enum class Sort { ServerOrder, MinutesDescending };

    struct Row {
        QString itemId;
        QString title;
        QString block;
        QString repeat;
        QString measure;
        QString scheduleText;
        QString nextOccurrence;
        int plannedDays = 0;
        int doneDays = 0;
        int minutes = 0;
        int count = 0;
        int target = 0;
        QStringList weekdays;
        StatsSeriesModel *series = nullptr;
    };

    explicit StatsItemsModel(QObject *parent = nullptr);
    ~StatsItemsModel() override;

    using TypedListModel::setRows;
    void setRows(const QVector<Row> &rows);
    void setItems(const std::vector<Dto::StatsItem> &items, Sort sort = Sort::MinutesDescending);

protected:
    static QVariantList values(const Row &row);

private:
    StatsSeriesModel *seriesFor(const QString &id);
    void pruneSeries(const QStringList &liveIds);

    QHash<QString, StatsSeriesModel *> m_series;
};
