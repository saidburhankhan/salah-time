#pragma once

#include "typedlistmodel.h"
#include "../client/dto.h"

// Preformatted chart axis ticks from the core, exposed as rows so the chart
// does not receive another QVariantList blob.
class StatsTicksModel : public TypedListModel {
    Q_OBJECT
    Q_PROPERTY(int lastMinutes READ lastMinutes NOTIFY lastMinutesChanged)
public:
    enum Role { MinutesRole = Qt::UserRole + 1, LabelRole };

    struct Row {
        int minutes = 0;
        QString label;
    };

    explicit StatsTicksModel(QObject *parent = nullptr);

    int lastMinutes() const { return m_lastMinutes; }
    Q_INVOKABLE QString labelAt(int row) const;
    Q_INVOKABLE int minutesAt(int row) const;

    using TypedListModel::setRows;
    void setRows(const QVector<Row> &rows);
    void setTicks(const std::vector<Dto::StatsTick> &ticks);

signals:
    void lastMinutesChanged();

private:
    static QVariantList values(const Row &row);
    int m_lastMinutes = 0;
};
