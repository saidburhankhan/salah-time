#pragma once

#include "typedlistmodel.h"

// One flat model for the Today timeline. Headers, item cards, the per-block
// add row, and the Sunrise/Sleep markers are all rows with a rowKind role, so
// the view has a single model and a single delegate switch.
class DayTimelineModel : public TypedListModel {
    Q_OBJECT
public:
    enum Role {
        RowKindRole = Qt::UserRole + 1,
        RowKeyRole,
        BlockRole,
        NameRole,
        TimeRole,
        MarkedRole,
        CurrentRole,
        ItemIdRole,
        TitleRole,
        StateRole,
        MeasureRole,
        RepeatRole,
        DetailTextRole,
        ScheduleTextRole,
        NextOccurrenceRole,
        CarriedRole,
        PendingRole,
        TimerRunningRole,
        TimerStartedAtRole,
        TimerMinutesRole,
        TimerProjectedRole,
        CountRole,
        TargetRole,
        WeekdaysRole,
        VersionRole,
        PositionRole
    };

    enum class Kind { Header, Item, Add, Marker };

    struct Row {
        Kind kind = Kind::Item;
        QString key;
        QString block;
        QString name;
        QString time;
        bool marked = false;
        bool current = false;
        QString itemId;
        QString title;
        QString state;
        QString measure;
        QString repeat;
        QString detailText;
        QString scheduleText;
        QString nextOccurrence;
        bool carried = false;
        bool pending = false;
        bool timerRunning = false;
        QString timerStartedAt;
        int timerMinutes = 0;
        bool timerProjected = false;
        int count = 0;
        int target = 0;
        QStringList weekdays;
        qint64 version = 0;
        int position = 0;
    };

    explicit DayTimelineModel(QObject *parent = nullptr);

    using TypedListModel::setRows;
    void setRows(const QVector<Row> &rows);

    Q_INVOKABLE QString kindAt(int row) const;
    Q_INVOKABLE QString keyAt(int row) const;
    Q_INVOKABLE QString blockAt(int row) const;
    Q_INVOKABLE QString itemIdAt(int row) const;
    Q_INVOKABLE QString titleAt(int row) const;
    Q_INVOKABLE QString detailTextAt(int row) const;
    Q_INVOKABLE QString stateAt(int row) const;
    Q_INVOKABLE int indexOfItem(const QString &itemId) const;

private:
    static QString kindName(Kind kind);
    static QVariantList values(const Row &row);
};
