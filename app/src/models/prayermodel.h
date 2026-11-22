#pragma once

#include "typedlistmodel.h"

class PrayerModel : public TypedListModel {
    Q_OBJECT
public:
    enum Role {
        PrayerIdRole = Qt::UserRole + 1,
        NameRole,
        TimeRole,
        PassedRole,
        CurrentRole,
        MarkedRole,
        PendingRole,
        VersionRole,
        DayRole
    };

    struct Row {
        QString prayerId;
        QString name;
        QString time;
        bool passed = false;
        bool current = false;
        bool marked = false;
        bool pending = false;
        qint64 version = 0;
        QString day;
    };

    explicit PrayerModel(QObject *parent = nullptr);

    using TypedListModel::setRows;
    void setRows(const QVector<Row> &rows);

private:
    static QVariantList values(const Row &row);
};
