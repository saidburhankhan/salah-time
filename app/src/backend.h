#pragma once

#include <QDate>
#include <QHash>
#include <QObject>
#include <QSet>
#include <QVariantMap>
#include <functional>
#include "client/coreclient.h"

class Backend : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString selectedDay READ selectedDay NOTIFY changed)
    Q_PROPERTY(QString todayDay READ todayDay NOTIFY changed)
    Q_PROPERTY(QVariantMap dayPlan READ dayPlan NOTIFY changed)
    Q_PROPERTY(QVariantMap stats READ stats NOTIFY changed)
    Q_PROPERTY(QVariantMap routinesStats READ routinesStats NOTIFY changed)
    Q_PROPERTY(QVariantMap config READ config NOTIFY changed)
    Q_PROPERTY(QVariantMap parseDraft READ parseDraft NOTIFY changed)
    Q_PROPERTY(QVariantMap preview READ preview NOTIFY previewChanged)
    Q_PROPERTY(qint64 dayPlanFetchedAt READ dayPlanFetchedAt NOTIFY changed)
    Q_PROPERTY(QString error READ error NOTIFY changed)
    Q_PROPERTY(bool busy READ busy NOTIFY changed)
    Q_PROPERTY(bool offlineMode READ offlineMode NOTIFY changed)
    Q_PROPERTY(int dayRevision READ dayRevision NOTIFY changed)
public:
    explicit Backend(QObject *parent = nullptr, const QString &program = {}, int timeoutMs = 10000,
                     const QString &socketPath = {});
    ~Backend() override;
    QString selectedDay() const { return m_selected.toString(Qt::ISODate); }
    QString todayDay() const { return m_today.toString(Qt::ISODate); }
    QVariantMap dayPlan() const { return m_dayPlan; }
    QVariantMap stats() const { return m_stats; }
    QVariantMap routinesStats() const { return m_routinesStats; }
    QVariantMap config() const { return m_config; }
    QVariantMap parseDraft() const { return m_parseDraft; }
    QVariantMap preview() const { return m_preview; }
    qint64 dayPlanFetchedAt() const { return m_dayPlanFetchedAt; }
    QString error() const { return m_error; }
    bool busy() const { return m_client.busy(); }
    bool offlineMode() const { return m_client.offline(); }
    int dayRevision() const { return m_dayRevision; }

    Q_INVOKABLE void reload();
    Q_INVOKABLE void loadStats(int days);
    Q_INVOKABLE void loadStatsRange(const QString &from, const QString &to);
    Q_INVOKABLE void moveDay(int amount);
    Q_INVOKABLE void selectDay(const QString &day);
    Q_INVOKABLE void jumpToday();
    Q_INVOKABLE void dismissError();
    Q_INVOKABLE void parseQuick(const QString &text);
    Q_INVOKABLE void previewItem(const QVariantMap &item);
    Q_INVOKABLE void addQuick(const QString &text, const QString &block = {});
    Q_INVOKABLE void saveItem(const QVariantMap &item);
    Q_INVOKABLE void removeItem(const QString &id);
    Q_INVOKABLE void moveItem(const QString &id, const QString &block,
                              const QString &afterId = {}, const QString &beforeId = {});
    Q_INVOKABLE void toggleDone(const QString &id, const QString &state);
    Q_INVOKABLE void adjustMinutes(const QString &id, int amount);
    Q_INVOKABLE void adjustCount(const QString &id, int amount);
    Q_INVOKABLE void toggleTimer(const QString &id, bool running);
    Q_INVOKABLE void togglePrayer(const QString &prayer, bool marked);
    Q_INVOKABLE void setSleepTime(const QString &time);
    Q_INVOKABLE QString hijriHeader(const QString &isoDate) const;
    Q_INVOKABLE QString gregorianHeader(const QString &isoDate) const;

signals:
    void changed();
    void previewChanged();
    void requestFinished(QStringList arguments, bool success);

private:
    void fail(const QString &message);
    static QVariantMap normalizedObject(const QJsonObject &object);
    bool setJsonModel(QVariantMap &target, QByteArray &previous, const QJsonObject &object);
    CoreClient::Callback response(std::function<void(const QJsonObject &)> success = {}, bool reloadAfter = false);
    CoreClient::Callback mutationResponse(const QString &pendingId = {}, const QString &pendingOpId = {});
    void applyMutation(const Dto::MutationResult &mutation);
    void setItemPending(const QString &id, bool pending);
    void setPrayerPending(const QString &prayer, bool pending);
    void insertPendingItem(const QVariantMap &item);
    void removePendingCreate(const QString &opId);
    void applyPendingFlags();
    qint64 versionFor(const QString &id) const;
    bool removeItemFromModel(const QString &id);
    int legacyMoveIndex(const QString &id, const QString &block, const QString &afterId, const QString &beforeId) const;
    void reloadFromNotification(qint64 revision);

    CoreClient m_client;
    QDate m_today = QDate::currentDate();
    QDate m_selected;
    QVariantMap m_dayPlan, m_stats, m_routinesStats, m_config, m_parseDraft, m_preview;
    qint64 m_dayPlanFetchedAt = 0;
    QByteArray m_dayJson, m_statsJson, m_routinesJson, m_configJson;
    QString m_error;
    bool m_refreshPending = false;
    int m_dayRevision = 0;
    qint64 m_lastNotificationRevision = -1;
    QHash<QString, qint64> m_versions;
    QSet<QString> m_pendingItems;
    QSet<QString> m_pendingPrayers;
    QHash<QString, QVariantMap> m_pendingCreates;
};
