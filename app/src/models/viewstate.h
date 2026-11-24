#pragma once

#include <QObject>
#include <QStringList>

#include "../client/dto.h"

// Small typed view states. They replace the former QVariantMap blobs that
// Backend exposed for singular objects; adding a field means adding one
// Q_PROPERTY here, not another QML map key.

class NowState : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString day READ day NOTIFY changed)
    Q_PROPERTY(QString block READ block NOTIFY changed)
    Q_PROPERTY(bool sleeping READ sleeping NOTIFY changed)
    Q_PROPERTY(QString nextId READ nextId NOTIFY changed)
    Q_PROPERTY(QString nextName READ nextName NOTIFY changed)
    Q_PROPERTY(QString nextTime READ nextTime NOTIFY changed)
    Q_PROPERTY(QString nextAt READ nextAt NOTIFY changed)
    Q_PROPERTY(qint64 nextRemainingSeconds READ nextRemainingSeconds NOTIFY changed)
    Q_PROPERTY(QString sunrise READ sunrise NOTIFY changed)
    Q_PROPERTY(QString nightStart READ nightStart NOTIFY changed)
    Q_PROPERTY(QString sleepTime READ sleepTime NOTIFY changed)
    Q_PROPERTY(bool sleepPassed READ sleepPassed NOTIFY changed)
    Q_PROPERTY(qint64 fetchedAt READ fetchedAt NOTIFY changed)
public:
    explicit NowState(QObject *parent = nullptr);

    QString day() const { return m_day; }
    QString block() const { return m_block; }
    bool sleeping() const { return m_sleeping; }
    QString nextId() const { return m_nextId; }
    QString nextName() const { return m_nextName; }
    QString nextTime() const { return m_nextTime; }
    QString nextAt() const { return m_nextAt; }
    qint64 nextRemainingSeconds() const { return m_nextRemainingSeconds; }
    QString sunrise() const { return m_sunrise; }
    QString nightStart() const { return m_nightStart; }
    QString sleepTime() const { return m_sleepTime; }
    bool sleepPassed() const { return m_sleepPassed; }
    qint64 fetchedAt() const { return m_fetchedAt; }

    void update(const Dto::DayPlan &plan, qint64 fetchedAt);

signals:
    void changed();

private:
    QString m_day, m_block, m_nextId, m_nextName, m_nextTime, m_nextAt;
    QString m_sunrise, m_nightStart, m_sleepTime;
    bool m_sleeping = false;
    bool m_sleepPassed = false;
    qint64 m_nextRemainingSeconds = 0;
    qint64 m_fetchedAt = 0;
};

class StatsTotals : public QObject {
    Q_OBJECT
    Q_PROPERTY(int planned READ planned NOTIFY changed)
    Q_PROPERTY(int done READ done NOTIFY changed)
    Q_PROPERTY(int minutes READ minutes NOTIFY changed)
    Q_PROPERTY(int count READ count NOTIFY changed)
    Q_PROPERTY(int prayers READ prayers NOTIFY changed)
    Q_PROPERTY(int activeDays READ activeDays NOTIFY changed)
    Q_PROPERTY(int prayerSlots READ prayerSlots NOTIFY changed)
    Q_PROPERTY(int days READ days NOTIFY changed)
public:
    explicit StatsTotals(QObject *parent = nullptr);

    int planned() const { return m_planned; }
    int done() const { return m_done; }
    int minutes() const { return m_minutes; }
    int count() const { return m_count; }
    int prayers() const { return m_prayers; }
    int activeDays() const { return m_activeDays; }
    int prayerSlots() const { return m_prayerSlots; }
    int days() const { return m_days; }

    void update(const Dto::Stats &stats);
    void set(int planned, int done, int minutes, int count, int prayers, int activeDays,
             int prayerSlots, int days);

signals:
    void changed();

private:
    int m_planned = 0, m_done = 0, m_minutes = 0, m_count = 0, m_prayers = 0;
    int m_activeDays = 0, m_prayerSlots = 0, m_days = 0;
};

class Config : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString location READ location NOTIFY changed)
    Q_PROPERTY(QString locationName READ locationName NOTIFY changed)
    Q_PROPERTY(QString method READ method NOTIFY changed)
    Q_PROPERTY(QString methodName READ methodName NOTIFY changed)
    Q_PROPERTY(QString sleepTime READ sleepTime NOTIFY changed)
    Q_PROPERTY(QString asr READ asr NOTIFY changed)
    Q_PROPERTY(bool autoLocation READ autoLocation NOTIFY changed)
    Q_PROPERTY(double latitude READ latitude NOTIFY changed)
    Q_PROPERTY(double longitude READ longitude NOTIFY changed)
    Q_PROPERTY(double timezoneOffset READ timezoneOffset NOTIFY changed)
    Q_PROPERTY(QString lastLocationProvider READ lastLocationProvider NOTIFY changed)
    Q_PROPERTY(QString lastLocationSync READ lastLocationSync NOTIFY changed)
public:
    explicit Config(QObject *parent = nullptr);

    QString location() const { return m_location; }
    QString locationName() const { return m_locationName; }
    QString method() const { return m_method; }
    QString methodName() const { return m_methodName; }
    QString sleepTime() const { return m_sleepTime; }
    QString asr() const { return m_asr; }
    bool autoLocation() const { return m_autoLocation; }
    double latitude() const { return m_latitude; }
    double longitude() const { return m_longitude; }
    double timezoneOffset() const { return m_timezoneOffset; }
    QString lastLocationProvider() const { return m_lastLocationProvider; }
    QString lastLocationSync() const { return m_lastLocationSync; }

    void update(const Dto::Config &config);

signals:
    void changed();

private:
    QString m_location, m_locationName, m_method, m_methodName, m_sleepTime, m_asr;
    QString m_lastLocationProvider, m_lastLocationSync;
    bool m_autoLocation = false;
    double m_latitude = 0, m_longitude = 0, m_timezoneOffset = 0;
};

class ParseDraftState : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString title READ title NOTIFY changed)
    Q_PROPERTY(QString block READ block NOTIFY changed)
    Q_PROPERTY(QString repeat READ repeat NOTIFY changed)
    Q_PROPERTY(int plannedMinutes READ plannedMinutes NOTIFY changed)
    Q_PROPERTY(bool hasPlannedMinutes READ hasPlannedMinutes NOTIFY changed)
    Q_PROPERTY(QStringList weekdays READ weekdays NOTIFY changed)
public:
    explicit ParseDraftState(QObject *parent = nullptr);

    QString title() const { return m_title; }
    QString block() const { return m_block; }
    QString repeat() const { return m_repeat; }
    int plannedMinutes() const { return m_plannedMinutes; }
    bool hasPlannedMinutes() const { return m_hasPlannedMinutes; }
    QStringList weekdays() const { return m_weekdays; }

    void update(const Dto::ParseDraft &draft);
    void clear();

signals:
    void changed();

private:
    QString m_title, m_block, m_repeat;
    int m_plannedMinutes = 0;
    bool m_hasPlannedMinutes = false;
    QStringList m_weekdays;
};

class PreviewState : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString nextOccurrence READ nextOccurrence NOTIFY changed)
    Q_PROPERTY(int occurrencesThisMonth READ occurrencesThisMonth NOTIFY changed)
    Q_PROPERTY(QString scheduleText READ scheduleText NOTIFY changed)
    Q_PROPERTY(bool valid READ valid NOTIFY changed)
public:
    explicit PreviewState(QObject *parent = nullptr);

    QString nextOccurrence() const { return m_nextOccurrence; }
    int occurrencesThisMonth() const { return m_occurrencesThisMonth; }
    QString scheduleText() const { return m_scheduleText; }
    bool valid() const { return m_valid; }

    void update(const Dto::Preview &preview);
    void clear();

signals:
    void changed();

private:
    QString m_nextOccurrence, m_scheduleText;
    int m_occurrencesThisMonth = 0;
    bool m_valid = false;
};

class ItemDraft : public QObject {
    Q_OBJECT
    Q_PROPERTY(qint64 version READ version NOTIFY changed)
    Q_PROPERTY(bool backlog READ backlog NOTIFY changed)
    Q_PROPERTY(QString id READ id NOTIFY changed)
    Q_PROPERTY(QString title READ title NOTIFY changed)
    Q_PROPERTY(QString block READ block NOTIFY changed)
    Q_PROPERTY(QString repeat READ repeat NOTIFY changed)
    Q_PROPERTY(QStringList weekdays READ weekdays NOTIFY changed)
    Q_PROPERTY(QString measure READ measure NOTIFY changed)
    Q_PROPERTY(int target READ target NOTIFY changed)
    Q_PROPERTY(QString projectId READ projectId NOTIFY changed)
    Q_PROPERTY(QString epicId READ epicId NOTIFY changed)
    Q_PROPERTY(QVariant metricIds READ metricIds NOTIFY changed)
    Q_PROPERTY(QVariant metrics READ metrics NOTIFY changed)
    Q_PROPERTY(QVariant tagIds READ tagIds NOTIFY changed)
    Q_PROPERTY(int plannedMinutes READ plannedMinutes NOTIFY changed)
public:
    explicit ItemDraft(QObject *parent = nullptr);

    QString id() const { return m_id; }
    qint64 version() const { return m_version; }
    bool backlog() const { return m_backlog; }
    QString title() const { return m_title; }
    QString block() const { return m_block; }
    QString repeat() const { return m_repeat; }
    QStringList weekdays() const { return m_weekdays; }
    QString measure() const { return m_measure; }
    int target() const { return m_target; }
    QString projectId() const { return m_projectId; }
    QString epicId() const { return m_epicId; }
    QVariant metricIds() const { return m_metricIds; }
    QVariant metrics() const { return m_metrics; }
    QVariant tagIds() const { return m_tagIds; }
    int plannedMinutes() const { return m_plannedMinutes; }

    void update(const QJsonObject &row);
    void update(const Dto::ParseDraft &draft);
    void clear();

signals:
    void changed();

private:
    QString m_id, m_title, m_block = QStringLiteral("fajr"), m_repeat = QStringLiteral("none");
    qint64 m_version = -1;
    bool m_backlog = false;
    QStringList m_weekdays;
    QString m_measure = QStringLiteral("check");
    int m_target = 0;
    QString m_projectId, m_epicId;
    QVariant m_metricIds;
    QVariant m_metrics;
    QVariant m_tagIds;
    int m_plannedMinutes = 0;
};
