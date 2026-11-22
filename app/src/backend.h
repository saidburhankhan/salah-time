#pragma once

#include <QDate>
#include <QObject>
#include <QVariantList>

class Backend : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString selectedDay READ selectedDay NOTIFY changed)
    Q_PROPERTY(QString todayDay READ todayDay NOTIFY changed)
    Q_PROPERTY(QString planningBlock READ planningBlock NOTIFY changed)
    Q_PROPERTY(QVariantList days READ days NOTIFY changed)
    Q_PROPERTY(QVariantList selectedItems READ selectedItems NOTIFY changed)
    Q_PROPERTY(QVariantList routines READ routines NOTIFY changed)
    Q_PROPERTY(QString error READ error NOTIFY changed)
    Q_PROPERTY(bool busy READ busy NOTIFY changed)
public:
    explicit Backend(QObject *parent = nullptr, const QString &program = {});
    QString selectedDay() const { return m_selected.toString(Qt::ISODate); }
    QString todayDay() const { return m_today.toString(Qt::ISODate); }
    QString planningBlock() const { return m_planningBlock; }
    QVariantList days() const { return m_days; }
    QVariantList selectedItems() const { return m_selectedItems; }
    QVariantList routines() const { return m_routines; }
    QString error() const { return m_error; }
    bool busy() const { return m_busy; }
    Q_INVOKABLE void reload();
    Q_INVOKABLE void moveDay(int amount);
    Q_INVOKABLE void selectDay(const QString &day);
    Q_INVOKABLE void jumpToday();
    Q_INVOKABLE void addItem(const QString &title, const QString &block, bool daily,
                             const QString &measure, int target);
    Q_INVOKABLE void toggleDone(const QString &id, const QString &state);
signals:
    void changed();
private:
    QByteArray run(const QStringList &arguments, bool *ok = nullptr);
    void rebuild(const QJsonArray &reports);
    QString m_program;
    QDate m_today, m_selected;
    QString m_planningBlock = "fajr", m_error;
    QVariantList m_days, m_selectedItems, m_routines;
    bool m_busy = false;
};
