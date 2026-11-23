#pragma once

#include <QDate>
#include <QHash>
#include <QJsonArray>
#include <QObject>
#include <QSet>
#include <QSortFilterProxyModel>
#include <functional>

#include "client/coreclient.h"
#include "models/daytimelinemodel.h"
#include "models/prayermodel.h"
#include "models/routinesmodel.h"
#include "models/statsdailymodel.h"
#include "models/statsitemsmodel.h"
#include "models/statsticksmodel.h"
#include "models/viewstate.h"

// Thin facade between CoreClient and QML. It owns the typed view models and
// singular view states, feeds them from checked DTOs, and exposes intent
// methods. It never hands a JSON blob to QML.
class Backend : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString selectedDay READ selectedDay NOTIFY selectedDayChanged)
    Q_PROPERTY(QString todayDay READ todayDay NOTIFY todayDayChanged)
    Q_PROPERTY(DayTimelineModel *dayTimeline READ dayTimeline CONSTANT)
    Q_PROPERTY(PrayerModel *prayers READ prayers CONSTANT)
    Q_PROPERTY(RoutinesModel *routines READ routines CONSTANT)
    Q_PROPERTY(StatsItemsModel *statsItems READ statsItems CONSTANT)
    Q_PROPERTY(StatsDailyModel *statsDaily READ statsDaily CONSTANT)
    Q_PROPERTY(QSortFilterProxyModel *statsDailyNewest READ statsDailyNewest CONSTANT)
    Q_PROPERTY(int statsDailyNewestCount READ statsDailyNewestCount NOTIFY statsDailyNewestCountChanged)
    Q_PROPERTY(StatsTicksModel *statsTicks READ statsTicks CONSTANT)
    Q_PROPERTY(NowState *nowState READ nowState CONSTANT)
    Q_PROPERTY(StatsTotals *statsTotals READ statsTotals CONSTANT)
    Q_PROPERTY(Config *config READ config CONSTANT)
    Q_PROPERTY(ParseDraftState *parseDraft READ parseDraft CONSTANT)
    Q_PROPERTY(PreviewState *preview READ preview CONSTANT)
    Q_PROPERTY(ItemDraft *editItem READ editItem NOTIFY editItemChanged)
    Q_PROPERTY(QString selectedStatsItem READ selectedStatsItem WRITE setSelectedStatsItem NOTIFY selectedStatsItemChanged)
    Q_PROPERTY(QString error READ error NOTIFY errorChanged)
    Q_PROPERTY(bool busy READ busy NOTIFY busyChanged)
    Q_PROPERTY(bool offlineMode READ offlineMode NOTIFY offlineChanged)
    Q_PROPERTY(int plannedItems READ plannedItems NOTIFY fulfilmentChanged)
    Q_PROPERTY(int doneItems READ doneItems NOTIFY fulfilmentChanged)
    Q_PROPERTY(int plannedMinutes READ plannedMinutes NOTIFY fulfilmentChanged)
    Q_PROPERTY(int actualMinutes READ actualMinutes NOTIFY fulfilmentChanged)
    Q_PROPERTY(int unplannedMinutes READ unplannedMinutes NOTIFY fulfilmentChanged)
    Q_PROPERTY(QVariant fulfilmentRatio READ fulfilmentRatio NOTIFY fulfilmentChanged)
    Q_PROPERTY(QVariantList projects READ projects NOTIFY settingsChanged)
    Q_PROPERTY(QVariantList tags READ tags NOTIFY settingsChanged)
    Q_PROPERTY(QVariantList metrics READ metrics NOTIFY settingsChanged)
    Q_PROPERTY(QVariantMap statsData READ statsData NOTIFY statsDataChanged)
    Q_PROPERTY(QVariantList trackingItems READ trackingItems NOTIFY trackingItemsChanged)
    Q_PROPERTY(QVariantMap running READ running NOTIFY runningChanged)
    Q_PROPERTY(QVariantList runningPrompt READ runningPrompt NOTIFY runningChanged)
    Q_PROPERTY(QString runningTitle READ runningTitle NOTIFY runningChanged)
    Q_PROPERTY(QString runningColor READ runningColor NOTIFY runningChanged)
    Q_PROPERTY(QVariantMap trends READ trends NOTIFY trendsChanged)
    Q_PROPERTY(QVariantMap statsTable READ statsTable NOTIFY statsTableChanged)
    Q_PROPERTY(QVariantList sessionsList READ sessionsList NOTIFY sessionsListChanged)
    Q_PROPERTY(bool sessionsHasMore READ sessionsHasMore NOTIFY sessionsListChanged)
    Q_PROPERTY(bool sessionsLoading READ sessionsLoading NOTIFY sessionsLoadingChanged)
public:
    explicit Backend(QObject *parent = nullptr, const QString &program = {}, int timeoutMs = 10000,
                     const QString &socketPath = {});
    ~Backend() override;

    QString selectedDay() const { return m_selected.toString(Qt::ISODate); }
    QString todayDay() const { return m_today.toString(Qt::ISODate); }
    DayTimelineModel *dayTimeline() { return &m_timeline; }
    PrayerModel *prayers() { return &m_prayers; }
    RoutinesModel *routines() { return &m_routines; }
    StatsItemsModel *statsItems() { return &m_statsItems; }
    StatsDailyModel *statsDaily() { return &m_statsDaily; }
    QSortFilterProxyModel *statsDailyNewest() { return &m_statsDailyNewest; }
    int statsDailyNewestCount() const { return m_statsDailyNewest.rowCount(); }
    StatsTicksModel *statsTicks() { return &m_ticks; }
    NowState *nowState() { return &m_now; }
    StatsTotals *statsTotals() { return &m_totals; }
    Config *config() { return &m_configState; }
    ParseDraftState *parseDraft() { return &m_parseDraft; }
    PreviewState *preview() { return &m_preview; }
    ItemDraft *editItem() { return m_editItem; }
    QString selectedStatsItem() const { return m_selectedStatsItem; }
    void setSelectedStatsItem(const QString &id);
    QString error() const { return m_error; }
    bool busy() const { return m_client.busy(); }
    bool offlineMode() const { return m_client.offline(); }
    int plannedItems() const { return m_dayPlan.json.value(QStringLiteral("fulfilment")).toObject().value(QStringLiteral("planned_items")).toInt(); }
    int doneItems() const { return m_dayPlan.json.value(QStringLiteral("fulfilment")).toObject().value(QStringLiteral("done_items")).toInt(); }
    int plannedMinutes() const { return m_dayPlan.json.value(QStringLiteral("fulfilment")).toObject().value(QStringLiteral("planned_minutes")).toInt(); }
    int actualMinutes() const { return m_dayPlan.json.value(QStringLiteral("fulfilment")).toObject().value(QStringLiteral("actual_minutes")).toInt(); }
    int unplannedMinutes() const { return m_dayPlan.json.value(QStringLiteral("fulfilment")).toObject().value(QStringLiteral("unplanned_minutes")).toInt(); }
    // Plan-line minutes over planned minutes, capped at 1; null when nothing
    // was planned (the day.get fulfilment carries the two sums only).
    QVariant fulfilmentRatio() const {
        const QJsonObject fulfilment = m_dayPlan.json.value(QStringLiteral("fulfilment")).toObject();
        const QJsonValue ratio = fulfilment.value(QStringLiteral("fulfilment_ratio"));
        if (ratio.isDouble()) return ratio.toDouble();
        const double planned = fulfilment.value(QStringLiteral("planned_minutes")).toDouble();
        if (planned <= 0) return QVariant();
        return qMin(1.0, fulfilment.value(QStringLiteral("actual_minutes")).toDouble() / planned);
    }
    QVariantList projects() const { return m_projects; }
    QVariantList tags() const { return m_tags; }
    QVariantList metrics() const { return m_metrics; }
    QVariantList trackingItems() const { return m_trackingItems; }
    QVariantMap statsData() const { return m_stats.json.toVariantMap(); }
    QVariantMap running() const { return m_running; }
    QVariantList runningPrompt() const { return m_runningPrompt; }
    QString runningTitle() const { return m_runningTitle; }
    QString runningColor() const { return m_runningColor; }
    QVariantMap trends() const { return m_trends; }
    QVariantMap statsTable() const { return m_statsTable; }
    QVariantList sessionsList() const { return m_sessionsList; }
    bool sessionsHasMore() const { return !m_sessionsNext.isEmpty(); }
    bool sessionsLoading() const { return m_sessionsLoading; }

    Q_INVOKABLE void reload();
    Q_INVOKABLE void loadStats(int days);
    Q_INVOKABLE void loadStatsFiltered(int days, const QString &tagId, const QString &projectId,
                                       const QString &itemId = {});
    Q_INVOKABLE void loadStatsRange(const QString &from, const QString &to, const QString &tagId = {},
                                    const QString &projectId = {}, const QString &itemId = {});
    // Reports (backend_reports.cpp). `filter` is the shared Statistics/Notes
    // filter: days or from/to, tag_id, project_id, item_id, minutes_min,
    // minutes_max, values {metric_id: {min?, max?}}; unknown keys are dropped.
    Q_INVOKABLE void loadStatsQuery(const QVariantMap &filter);
    Q_INVOKABLE void loadTrends(const QVariantMap &filter, const QStringList &measures,
                                const QString &interval, const QString &split);
    Q_INVOKABLE void loadStatsTable(const QVariantMap &filter, const QString &group = {});
    // sessions.list: the filter plus with_note and q. loadMoreSessions pages
    // with the last `next` cursor and the same filter.
    Q_INVOKABLE void loadSessions(const QVariantMap &filter);
    Q_INVOKABLE void loadMoreSessions();
    // Small UI preferences (table columns) kept in QSettings.
    Q_INVOKABLE QVariant uiSetting(const QString &key, const QVariant &fallback = {}) const;
    Q_INVOKABLE void setUiSetting(const QString &key, const QVariant &value);
    Q_INVOKABLE void loadTrackingItems();
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
    // Done is an explicit tick: an item with a tick today is unticked (its
    // tick sessions removed), otherwise ticked. Timed sessions never count.
    Q_INVOKABLE bool toggleDone(const QString &id, const QString &state = {});
    Q_INVOKABLE void tickItem(const QString &id, const QVariantMap &values = {}, const QString &note = {});
    Q_INVOKABLE void untickItem(const QString &id);
    Q_INVOKABLE void logSession(const QString &itemId, const QString &projectId, int minutes,
                                const QVariantMap &values = {}, const QString &note = {});
    Q_INVOKABLE void startSession(const QString &itemId, const QString &projectId = {});
    Q_INVOKABLE void stopSession(const QString &sessionId, const QVariantMap &values = {},
                                 const QString &note = {}, bool done = false, int minutes = -1);
    Q_INVOKABLE void editSession(const QString &sessionId, int minutes, const QVariantMap &values,
                                 const QString &note);
    Q_INVOKABLE void removeSession(const QString &sessionId);
    Q_INVOKABLE void bumpDistractions(const QString &sessionId, int delta);
    Q_INVOKABLE void addRunningNote(const QString &sessionId, const QString &text);
    Q_INVOKABLE void toggleTimer(const QString &id, bool running);
    Q_INVOKABLE void togglePrayer(const QString &prayer, bool marked);
    Q_INVOKABLE void setSleepTime(const QString &time);
    Q_INVOKABLE void loadSettings();
    Q_INVOKABLE void saveEntity(const QString &kind, const QVariantMap &fields);
    Q_INVOKABLE void removeTag(const QString &id);
    Q_INVOKABLE ItemDraft *itemForId(const QString &id);
    Q_INVOKABLE ItemDraft *itemFromParse(const QString &fallbackTitle = {});
    Q_INVOKABLE QString hijriHeader(const QString &isoDate) const;
    Q_INVOKABLE QString gregorianHeader(const QString &isoDate) const;

signals:
    void selectedDayChanged();
    void todayDayChanged();
    void selectedStatsItemChanged();
    void statsDailyNewestCountChanged();
    void editItemChanged();
    void errorChanged();
    void busyChanged();
    void offlineChanged();
    void requestFinished(QStringList arguments, bool success);
    void fulfilmentChanged();
    void settingsChanged();
    void statsDataChanged();
    void trackingItemsChanged();
    // A project/tag/metric write finished. `id` is the saved row's id (empty
    // for a removal); on failure `error` carries the server message.
    void entitySaved(const QString &kind, const QString &id);
    void entitySaveFailed(const QString &kind, const QString &error);
    // A session write (tick, log, stop, edit, remove) finished.
    void sessionWriteFinished(bool ok, const QString &message);
    void runningNoteFinished(const QString &sessionId, bool ok);
    void runningChanged();
    void trendsChanged();
    void statsTableChanged();
    void sessionsListChanged();
    void sessionsLoadingChanged();

private:
    struct PendingCreate {
        QString opId;
        QString localId;
        QString title;
        QString block;
        QString repeat = QStringLiteral("none");
        QString projectId;
        int plannedMinutes = 0;
        QStringList weekdays;
    };

    void fail(const QString &message);
    void clearError();
    CoreClient::Callback readResponse(std::function<void(const CoreClient::Result &)> success,
                                      bool reloadAfter = false);
    CoreClient::Callback mutationResponse(const QString &pendingId = {}, const QString &pendingOpId = {});
    CoreClient::Callback entityResponse(const QString &kind);
    CoreClient::Callback sessionResponse(const QString &pendingItemId, bool announce = true);
    CoreClient::Callback statsResponse();
    void applyRunningWrite(const CoreClient::Result &result);
    void applyStatus(const QJsonObject &status);
    static QJsonObject filterParams(const QVariantMap &filter, bool allowAnyDays);
    void requestSessions(const QJsonObject &params, bool append);
    void refreshRunning();
    QJsonArray sessionsOf(const QString &itemId) const;
    QDate writeDate() const;
    void applyDayPlan(const Dto::DayPlan &plan);
    void applyStats(const Dto::Stats &stats);
    void applyRoutines(const Dto::Stats &stats);
    void applyConfig(const Dto::Config &config);
    void applyMutation(const Dto::MutationResult &mutation);
    void rebuildTimeline();
    void rebuildPrayers();
    void setItemPending(const QString &id, bool pending);
    void setPrayerPending(const QString &prayer, bool pending);
    void insertPending(const PendingCreate &pending);
    void removePendingCreate(const QString &opId);
    void optimisticMove(const QString &id, const QString &block, const QString &afterId,
                        const QString &beforeId);
    qint64 versionFor(const QString &id) const;
    int legacyMoveIndex(const QString &id, const QString &block, const QString &afterId,
                        const QString &beforeId) const;
    void reloadFromNotification(qint64 revision);
    bool dayContains(const QString &id) const;

    CoreClient m_client;
    QDate m_today = QDate::currentDate();
    QDate m_selected;
    Dto::DayPlan m_dayPlan;
    Dto::Stats m_stats;
    Dto::Stats m_routinesStats;
    Dto::Config m_config;
    Dto::ParseDraft m_parse;
    qint64 m_dayPlanFetchedAt = 0;
    QString m_error;
    QString m_selectedStatsItem;
    bool m_refreshPending = false;
    qint64 m_lastNotificationRevision = -1;
    QHash<QString, qint64> m_versions;
    QSet<QString> m_pendingItems;
    QSet<QString> m_pendingPrayers;
    QHash<QString, PendingCreate> m_pendingCreates;
    ItemDraft *m_editItem = nullptr;

    DayTimelineModel m_timeline;
    PrayerModel m_prayers;
    RoutinesModel m_routines;
    StatsItemsModel m_statsItems;
    StatsDailyModel m_statsDaily;
    QSortFilterProxyModel m_statsDailyNewest;
    StatsTicksModel m_ticks;
    NowState m_now;
    StatsTotals m_totals;
    Config m_configState;
    ParseDraftState m_parseDraft;
    PreviewState m_preview;
    QVariantList m_projects, m_tags, m_metrics;
    QVariantList m_trackingItems;
    quint64 m_statsRequest = 0;
    QVariantMap m_running, m_lastRunning;
    QVariantList m_runningPrompt, m_lastRunningPrompt;
    QString m_runningTitle, m_runningColor;
    QVariantMap m_trends;
    quint64 m_trendsRequest = 0;
    QVariantMap m_statsTable;
    quint64 m_statsTableRequest = 0;
    QVariantList m_sessionsList;
    QJsonObject m_sessionsParams;
    QString m_sessionsNext;
    quint64 m_sessionsRequest = 0;
    bool m_sessionsLoading = false;
};
