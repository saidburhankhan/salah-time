#include <QDir>
#include <cstring>
#include <memory>
#include <QElapsedTimer>
#include <QFileInfo>
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QAbstractItemModelTester>
#include <QLocalServer>
#include <QLocalSocket>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QQmlComponent>
#include <QQmlContext>
#include <QQmlEngine>
#include <QQuickItem>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QSet>
#include <QSettings>
#include <QtTest>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>
#include "backend.h"
#include "client/coreclient.h"
#include "client/dto.h"
#include "models/statsseriesmodel.h"
#include "launch.h"
#include "theme.h"
#include "translator.h"
#include "tracking_fixture.h"

namespace {
class FakeDaemon : public QObject {
public:
    using Handler = std::function<void(const QJsonObject &, QLocalSocket *, FakeDaemon *)>;
    explicit FakeDaemon(const QString &path) : m_path(path) {
        QLocalServer::removeServer(path);
        m_server.listen(path);
        connect(&m_server, &QLocalServer::newConnection, this, [this] { onNewConnection(); });
    }
    ~FakeDaemon() override {
        const auto clients = m_clients;
        for (QLocalSocket *socket : clients) {
            socket->disconnect(this);
            socket->abort();
            delete socket;
        }
        m_clients.clear();
        m_buffers.clear();
        m_server.close();
        QLocalServer::removeServer(m_path);
    }
    bool ok() const { return m_server.isListening(); }
    void setHandler(Handler handler) { m_handler = std::move(handler); }
    void respond(QLocalSocket *socket, const QJsonObject &request, const QJsonValue &result) {
        write(socket, QJsonObject{{"jsonrpc", "2.0"}, {"id", request.value("id")}, {"result", result}});
    }
    void respondError(QLocalSocket *socket, const QJsonObject &request, int code, const QString &kind, const QString &message,
                      QJsonObject data = {}) {
        data.insert("kind", kind);
        data.insert("retryable", false);
        write(socket, QJsonObject{{"jsonrpc", "2.0"}, {"id", request.value("id")},
                                  {"error", QJsonObject{{"code", code}, {"message", message}, {"data", data}}}});
    }
    void notifyChanged(qint64 revision, const QStringList &scopes = {"items"}) {
        QJsonArray scopeArray;
        for (const QString &scope : scopes) scopeArray.append(scope);
        const QJsonObject frame{{"jsonrpc", "2.0"}, {"method", "changed"},
                                {"params", QJsonObject{{"revision", revision}, {"scopes", scopeArray}, {"actor", "test"}}}};
        const QByteArray line = QJsonDocument(frame).toJson(QJsonDocument::Compact) + '\n';
        for (QLocalSocket *socket : std::as_const(m_clients)) socket->write(line);
    }
    void notifyClock(const QString &day) {
        const QJsonObject frame{{"jsonrpc", "2.0"}, {"method", "clock.boundary"}, {"params", QJsonObject{{"day", day}}}};
        for (QLocalSocket *socket : std::as_const(m_clients)) write(socket, frame);
    }
    void dropClients() {
        const auto clients = m_clients;
        for (QLocalSocket *socket : clients) socket->disconnectFromServer();
    }
    QList<QJsonObject> requests;
    int hellos = 0;
    int subscribes = 0;
    QJsonObject helloResult{{"protocol", 1}, {"server", "salah test"},
                            {"capabilities", QJsonArray{"tracking.v1", "events.v1", "clock.v1"}}, {"revision", 100}};
    QJsonObject subscribeResult{{"revision", 100}};
private:
    void write(QLocalSocket *socket, const QJsonObject &object) {
        if (!socket) return;
        socket->write(QJsonDocument(object).toJson(QJsonDocument::Compact) + '\n');
        socket->flush();
    }
    void onNewConnection() {
        while (QLocalSocket *socket = m_server.nextPendingConnection()) {
            m_clients.append(socket);
            m_buffers.insert(socket, {});
            connect(socket, &QLocalSocket::readyRead, this, [this, socket] { onReadyRead(socket); });
            connect(socket, &QLocalSocket::disconnected, this, [this, socket] {
                m_clients.removeAll(socket);
                m_buffers.remove(socket);
                socket->deleteLater();
            });
        }
    }
    void onReadyRead(QLocalSocket *socket) {
        QByteArray &buffer = m_buffers[socket];
        buffer += socket->readAll();
        for (;;) {
            const int newline = buffer.indexOf('\n');
            if (newline < 0) break;
            const QByteArray line = buffer.left(newline);
            buffer.remove(0, newline + 1);
            if (line.trimmed().isEmpty()) continue;
            const QJsonObject request = QJsonDocument::fromJson(line).object();
            requests.append(request);
            const QString method = request.value("method").toString();
            if (method == "hello") {
                ++hellos;
                respond(socket, request, helloResult);
                continue;
            }
            if (method == "subscribe") {
                ++subscribes;
                respond(socket, request, subscribeResult);
                continue;
            }
            if (m_handler) m_handler(request, socket, this);
            else respond(socket, request, QJsonObject{});
        }
    }
    QLocalServer m_server;
    QString m_path;
    QList<QLocalSocket *> m_clients;
    QHash<QLocalSocket *, QByteArray> m_buffers;
    Handler m_handler;
};

// Fixtures for rendering the pages even when the sandbox cannot bind a socket.
class TrackingUiPreviewBackend : public Backend {
    Q_OBJECT
    Q_PROPERTY(bool offlineMode READ previewOfflineMode CONSTANT)
    Q_PROPERTY(QVariantList projects READ previewProjects CONSTANT)
    Q_PROPERTY(QVariantList tags READ previewTags NOTIFY tagsLoaded)
    Q_PROPERTY(QVariantList epics READ previewEpics CONSTANT)
    Q_PROPERTY(QVariantList metrics READ previewMetrics CONSTANT)
    Q_PROPERTY(QVariantList trackingItems MEMBER m_previewTasks CONSTANT)
    Q_PROPERTY(QVariantList backlogItems MEMBER m_previewBacklog CONSTANT)
    Q_PROPERTY(QVariantMap statsTable MEMBER m_previewStatsTable CONSTANT)
public:
    TrackingUiPreviewBackend(const QString &program, const QString &socketPath,
                             const QJsonArray &tasks, const QJsonArray &backlog,
                             const QJsonObject &statsTable = {})
        : Backend(nullptr, program, 3000, socketPath),
          m_previewTasks(tasks.toVariantList()), m_previewBacklog(backlog.toVariantList()),
          m_previewStatsTable(statsTable.toVariantMap()) {}
    bool previewOfflineMode() const { return false; }
    QVariantList previewProjects() const { return TrackingFixture::projects().toVariantList(); }
    QVariantList tagCatalog = TrackingFixture::tags().toVariantList();
    QVariantList previewTags() const { return tagCatalog; }
    QVariantList epicCatalog = TrackingFixture::epics().toVariantList();
    QVariantList previewEpics() const { return epicCatalog; }
    QVariantList previewMetrics() const { return TrackingFixture::metrics().toVariantList(); }
private:
    const QVariantList m_previewTasks;
    const QVariantList m_previewBacklog;
    const QVariantMap m_previewStatsTable;
};

class ReportsPreviewBackend : public TrackingUiPreviewBackend {
    Q_OBJECT
    Q_PROPERTY(QString todayDay READ trackingDate NOTIFY todayDayChanged)
    Q_PROPERTY(QVariantMap insights MEMBER report NOTIFY previewChanged)
    Q_PROPERTY(QVariantMap review MEMBER reviewData NOTIFY previewChanged)
    Q_PROPERTY(QVariantList goals MEMBER goalRows CONSTANT)
    Q_PROPERTY(QString goalHistoryId MEMBER historyId CONSTANT)
    Q_PROPERTY(QVariantList goalHistory MEMBER history CONSTANT)
public:
    using TrackingUiPreviewBackend::TrackingUiPreviewBackend;
    QString date = "2031-05-19";
    QString trackingDate() const { return date; }
    QStringList reads;
    QString submittedWeek, submittedNote;
    Q_INVOKABLE void loadReview(const QString &week) { reads.append(week); }
    Q_INVOKABLE void saveReview(const QString &week, const QString &note) { submittedWeek = week; submittedNote = note; }
    QVariantMap report, reviewData;
    QVariantList goalRows, history;
    QString historyId;
signals:
    void previewChanged();
};

}

class SalahAppTests : public QObject {
    Q_OBJECT
    static void write(const QString &path, const QByteArray &data) { QFile f(path); QVERIFY(f.open(QIODevice::WriteOnly)); QCOMPARE(f.write(data), data.size()); }
    static QJsonDocument readJson(const QString &path) { QFile f(path); if (!f.open(QIODevice::ReadOnly)) qFatal("fixture not readable"); return QJsonDocument::fromJson(f.readAll()); }
    static QByteArray fakeRpc(const QTemporaryDir &dir) {
        return "if [ \"$1\" = rpc ]; then\n"
               "  IFS= read -r hello && IFS= read -r request || exit 2\n"
               "  printf '%s\\n' \"$hello\" \"$request\" >> '" + dir.filePath("frames").toUtf8() + "'\n"
               "  printf '%s\\n' \"$request\" | jq -c 'if .params.id == \"conflicting-edit\" then "
               "{jsonrpc,id,error:{code:-32004,message:\"Session was changed\",data:{kind:\"conflict\"}}} else "
               "{jsonrpc,id,result:{revision:1,op_id:.params.op_id,"
               "changed:{items:[],sessions:[],projects:[],tags:[],metrics:[],marks:[],deleted:[]}}} end'\n"
               "  exit 0\nfi\n";
    }
    static QList<QJsonObject> offlineRequests(const QTemporaryDir &dir) {
        QFile frames(dir.filePath("frames"));
        QList<QJsonObject> requests;
        if (frames.open(QIODevice::ReadOnly)) {
            while (!frames.atEnd()) {
                const auto frame = QJsonDocument::fromJson(frames.readLine()).object();
                if (frame.value("id").toInt() == 2) requests.append(frame);
            }
        }
        return requests;
    }
    static QString fake(QTemporaryDir &dir) {
        const QString path=dir.filePath("salah"), log=dir.filePath("args");
        QByteArray script = "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '"+log.toUtf8()+"'\n";
        script += fakeRpc(dir);
        script += "rpc='" TEST_SOURCE_DIR "/../../crates/salah-cli/tests/fixtures/rpc/'\n"
                  "if [ \"$1 $2\" = \"item list\" ]; then jq -c .result \"${rpc}items_list.json\"; exit 0; fi\n"
                  "if [ \"$1\" = day ]; then jq -c .result \"${rpc}day_tracking.json\"; exit 0; fi\n"
                  "if [ \"$1\" = stats ]; then jq -c .result \"${rpc}stats.json\"; exit 0; fi\n";
        const QByteArray status = dir.filePath("status.json").toUtf8();
        script += "if [ \"$1\" = status ] && [ -f '" + status + "' ]; then cat '" + status + "'; exit 0; fi\n";
        script += R"SH(
if [ "$5" = "slow" ]; then sleep 2; printf '{}\n'; exit 0; fi
if [ "$1 $2" = "item done" ]; then sleep 1; printf '{}\n'; exit 0; fi
if [ "$1" = "day" ]; then
 printf '%s\n' '{"day":"2026-11-22","is_today":true,"now":{"block":"dhuhr","sleeping":false,"next":{"id":"asr","name":"Asr","time":"15:18","remaining_seconds":7200}},"prayers":[{"id":"fajr","name":"Fajr","time":"05:58","passed":true,"current":false,"marked":true},{"id":"dhuhr","name":"Dhuhr","time":"12:11","passed":false,"current":true,"marked":false},{"id":"asr","name":"Asr","time":"15:18","passed":false,"current":false,"marked":false},{"id":"maghrib","name":"Maghrib","time":"17:00","passed":false,"current":false,"marked":false},{"id":"isha","name":"Isha","time":"18:20","passed":false,"current":false,"marked":false}],"sunrise":"07:16","sleep":{"time":"23:00","passed":false},"night_start":"23:00","blocks":[{"start":"night","end":"fajr","items":[{"id":"n1","title":"Tahajjud","block":"night","repeat":"daily","weekdays":null,"measure":"check","target":null,"position":0,"state":"open","minutes":3,"count":0,"timer_started_at":"2026-11-22T08:00:00Z"}]},{"start":"fajr","end":"dhuhr","items":[{"id":"a1","title":"Read","block":"fajr","repeat":"daily","weekdays":null,"measure":"minutes","target":20,"position":0,"state":"done","minutes":15,"count":0,"timer_started_at":null}]},{"start":"dhuhr","end":"asr","items":[]},{"start":"asr","end":"maghrib","items":[]},{"start":"maghrib","end":"isha","items":[]},{"start":"isha","end":"sleep","items":[]}],"open":1}'
elif [ "$1" = "stats" ]; then
 printf '%s\n' '{"from":"2026-11-09","to":"2026-11-22","days":14,"items":[{"id":"a1","title":"Read","block":"fajr","repeat":"daily","weekdays":null,"measure":"minutes","target":20,"planned_days":14,"done_days":9,"minutes":180,"count":0,"series":[{"day":"2026-11-22","planned":true,"done":true,"minutes":25,"count":0}]}],"prayers":[{"prayer":"fajr","marked_days":10,"days":14}],"marked_total":10,"prayer_slots":70,"daily":[{"day":"2026-11-22","planned":1,"done":1,"minutes":25,"count":0,"prayers":4}],"totals":{"planned":14,"done":9,"minutes":180,"count":0,"prayers":10,"active_days":11}}'
elif [ "$1 $2" = "config show" ]; then printf '%s\n' '{"asr":"hanafi","auto_location":true,"last_location_provider":null,"last_location_sync":null,"latitude":41.2,"location":"Tashkent","longitude":69.2,"method":"uzbekistan","sleep_time":"23:00","timezone_offset":5}'
elif [ "$1 $2" = "item parse" ]; then printf '%s\n' '{"title":"Read","block":"isha","repeat":"daily","weekdays":null,"measure":"minutes","target":20}'
else printf '%s\n' '{}'; fi
)SH";
        write(path,script); QFile::setPermissions(path,QFile::ReadOwner|QFile::WriteOwner|QFile::ExeOwner); return path;
    }
    static QStringList logLines(QTemporaryDir &dir) { QFile f(dir.filePath("args")); if (!f.open(QIODevice::ReadOnly)) return {}; return QString::fromUtf8(f.readAll()).split('\n',Qt::SkipEmptyParts); }
    static QString fakeCore(QTemporaryDir &dir) {
        const QString path = dir.filePath("core-salah"), log = dir.filePath("core-args");
        QByteArray script = "#!/bin/sh\nfor arg in \"$@\"; do printf '<%s>' \"$arg\"; done >> '" + log.toUtf8() + "'\nprintf '\\n' >> '" + log.toUtf8() + "'\n";
        script += fakeRpc(dir);
        script += QByteArray(R"SH(
case "$*" in
  *2099-01-01*) sleep 1 ;;
  *malformed*) printf 'not-json\n'; exit 0 ;;
  *failure*) printf 'salah: requested failure\n' >&2; exit 7 ;;
esac
)SH") + "fixture='" TEST_SOURCE_DIR "/../../crates/salah-cli/tests/fixtures/rpc/'\n" + R"SH(
case "$1 $2" in
  'day --date') jq -c .result "${fixture}day_tracking.json" ;;
  'stats --days'|'stats --from') jq -c .result "${fixture}stats.json" ;;
  'config show') printf '%s\n' '{"asr":"hanafi","auto_location":true,"last_location_provider":null,"last_location_sync":null,"latitude":41.2,"location":"Tashkent","longitude":69.2,"method":"uzbekistan","sleep_time":"23:00","timezone_offset":5}' ;;
  *) printf '{"ok":true}\n' ;;
esac
)SH";
        write(path, script);
        QFile::setPermissions(path, QFile::ReadOwner | QFile::WriteOwner | QFile::ExeOwner);
        return path;
    }
    static QStringList coreLogLines(QTemporaryDir &dir) {
        QFile file(dir.filePath("core-args"));
        if (!file.open(QIODevice::ReadOnly)) return {};
        return QString::fromUtf8(file.readAll()).split('\n', Qt::SkipEmptyParts);
    }
    static bool hasLabelText(QObject *root, const QString &text) {
        for (QObject *object : root->findChildren<QObject *>())
            if (object->property("text").toString() == text) return true;
        return false;
    }
    static int timelineItem(DayTimelineModel *model, const QString &id) {
        for (int row = 0; row < model->rowCount(); ++row)
            if (model->data(model->index(row, 0), DayTimelineModel::RowKindRole).toString() == "item" &&
                model->data(model->index(row, 0), DayTimelineModel::ItemIdRole).toString() == id)
                return row;
        return -1;
    }
    static int timelineTitle(DayTimelineModel *model, const QString &title) {
        for (int row = 0; row < model->rowCount(); ++row)
            if (model->data(model->index(row, 0), DayTimelineModel::RowKindRole).toString() == "item" &&
                model->data(model->index(row, 0), DayTimelineModel::TitleRole).toString() == title)
                return row;
        return -1;
    }
    // A day with items in several blocks and one deliberately empty block, so
    // the drag tests exercise both in-block and cross-block drops.
    static QJsonObject dragDay() {
        QJsonObject day = fixture("day");
        QJsonArray blocks = day.value("blocks").toArray();
        for (int b = 0; b < blocks.size(); ++b) {
            QJsonObject block = blocks[b].toObject();
            if (block.value("start").toString() != "fajr") continue;
            QJsonArray items = block.value("items").toArray();
            if (items.isEmpty()) return day;
            QJsonObject second = items.first().toObject();
            second["id"] = "fajr-2";
            second["title"] = "Fajr second";
            second["position"] = 1;
            items.append(second);
            block["items"] = items;
            blocks[b] = block;
            break;
        }
        day["blocks"] = blocks;
        return day;
    }
    static QJsonObject movedDay(QJsonObject day, const QString &id, const QString &targetBlock, int index) {
        QJsonArray blocks = day.value("blocks").toArray();
        QJsonObject moving;
        for (int b = 0; b < blocks.size(); ++b) {
            QJsonObject block = blocks[b].toObject();
            QJsonArray rows = block.value("items").toArray();
            for (int i = 0; i < rows.size(); ++i) {
                if (rows[i].toObject().value("id").toString() != id) continue;
                moving = rows.takeAt(i).toObject(); block["items"] = rows; blocks[b] = block; break;
            }
        }
        if (moving.isEmpty()) return day;
        for (int b = 0; b < blocks.size(); ++b) {
            QJsonObject block = blocks[b].toObject();
            if (block.value("start").toString() != targetBlock) continue;
            QJsonArray rows = block.value("items").toArray();
            const int at = qBound(0, index, rows.size());
            moving["block"] = targetBlock; moving["position"] = at; rows.insert(at, moving);
            for (int i = 0; i < rows.size(); ++i) { QJsonObject row = rows[i].toObject(); row["position"] = i; rows[i] = row; }
            block["items"] = rows; blocks[b] = block; break;
        }
        day["blocks"] = blocks;
        return day;
    }
    static QJsonObject movedDay(QJsonObject day, const QJsonObject &params) {
        int index = params.value("index").toInt();
        const QString target = params.value("block").toString();
        const QString anchor = params.contains("after_id") ? params.value("after_id").toString()
                                                            : params.value("before_id").toString();
        if (!anchor.isEmpty()) {
            for (const QJsonValue &blockValue : day.value("blocks").toArray()) {
                const QJsonObject block = blockValue.toObject();
                if (block.value("start").toString() != target) continue;
                const QJsonArray rows = block.value("items").toArray();
                for (int i = 0; i < rows.size(); ++i)
                    if (rows[i].toObject().value("id").toString() == anchor)
                        index = i + (params.contains("after_id") ? 1 : 0);
            }
        }
        return movedDay(day, params.value("id").toString(), target, index);
    }
    static QQuickItem *timelineView(QObject *root) {
        for (QObject *object : root->findChildren<QObject *>()) {
            auto *item = qobject_cast<QQuickItem *>(object);
            if (item && QByteArray(item->metaObject()->className()).contains("QQuickListView"))
                return item;
        }
        return nullptr;
    }
    static void appendQuickItems(QQuickItem *parent, QList<QQuickItem *> &items) {
        for (QQuickItem *child : parent->childItems()) {
            items.append(child);
            appendQuickItems(child, items);
        }
    }
    static QList<QQuickItem *> quickDescendants(QQuickItem *root) {
        QList<QQuickItem *> items;
        appendQuickItems(root, items);
        return items;
    }
    static QQuickItem *rowDelegate(QQuickItem *list, int row) {
        auto *content = list->property("contentItem").value<QQuickItem *>();
        if (!content) return nullptr;
        for (QQuickItem *item : content->childItems())
            if (item->property("index").toInt() == row && item->property("rowKind").isValid())
                return item;
        return nullptr;
    }
    static QQuickItem *itemDelegate(QQuickItem *list, const QString &id) {
        auto *content = list->property("contentItem").value<QQuickItem *>();
        if (!content) return nullptr;
        for (QQuickItem *item : quickDescendants(content))
            if (item->property("itemId").toString() == id && item->property("rowIndex").isValid())
                return item;
        return nullptr;
    }
    static QQuickItem *dragHandle(QQuickItem *card) {
        for (QQuickItem *item : quickDescendants(card)) {
            if (item->property("text").toString() != QString::fromUtf8("⠿")) continue;
            for (QQuickItem *child : quickDescendants(item))
                if (child->property("pressed").isValid()) return child;
        }
        return nullptr;
    }
    static QPoint sceneCenter(QQuickItem *item, qreal yFraction = 0.5) {
        return item->mapToScene(QPointF(item->width() / 2, item->height() * yFraction)).toPoint();
    }
    static QStringList timelineKeys(DayTimelineModel *model) {
        QStringList keys;
        for (int row = 0; row < model->rowCount(); ++row) keys.append(model->keyAt(row));
        return keys;
    }
    static int rowForKey(DayTimelineModel *model, const QString &key) {
        for (int row = 0; row < model->rowCount(); ++row)
            if (model->keyAt(row) == key) return row;
        return -1;
    }
    static QJsonObject lastRequest(const FakeDaemon &daemon, const QString &method) {
        for (int i = daemon.requests.size() - 1; i >= 0; --i) {
            const QJsonObject &request = daemon.requests.at(i);
            if (request.value("method").toString() == method) return request;
        }
        return {};
    }
    static int requestCount(const FakeDaemon &daemon, const QString &method) {
        int count = 0;
        for (const QJsonObject &request : daemon.requests)
            if (request.value("method").toString() == method) ++count;
        return count;
    }
    // Showing/activating the window makes Main.qml reload the day. Let those
    // requests land before a drag starts, so a late rebuild cannot reorder the
    // list under the pointer.
    static void waitForBackendIdle(Backend &backend) {
        QTest::qWait(200);
        QTRY_VERIFY_WITH_TIMEOUT(!backend.busy(), 3000);
        QTest::qWait(100);
        QTRY_VERIFY_WITH_TIMEOUT(!backend.busy(), 3000);
    }
    static int prayerRow(PrayerModel *model, const QString &id) {
        for (int row = 0; row < model->rowCount(); ++row)
            if (model->data(model->index(row, 0), PrayerModel::PrayerIdRole).toString() == id) return row;
        return -1;
    }
    static QVariantMap itemEditMap(Backend &backend, const QString &id) {
        ItemDraft *draft = backend.itemForId(id);
        if (!draft) return {};
        QVariantMap item{{"id", draft->id()}, {"title", draft->title()}, {"block", draft->block()},
                         {"repeat", draft->repeat()}, {"weekdays", draft->weekdays()}};
        return item;
    }

    static QJsonObject fixture(const QString &name) {
        QString file = name;
        if (name == "day") file = "day_tracking";
        else if (name == "stats_days" || name == "stats_range") file = "stats";
        const QJsonObject envelope = readJson(QStringLiteral(TEST_SOURCE_DIR "/../../crates/salah-cli/tests/fixtures/rpc/") + file + ".json").object();
        return envelope.value("result").toObject();
    }
    static FakeDaemon::Handler readHandler(QJsonObject day) {
        return [day](const QJsonObject &request, QLocalSocket *socket, FakeDaemon *daemon) {
            const QString method = request.value("method").toString();
            if (method == "day.get") daemon->respond(socket, request, day);
            else if (method == "stats.get") daemon->respond(socket, request, QJsonValue(fixture("stats_days")));
            else if (method == "config.get") daemon->respond(socket, request, QJsonObject{{"sleep_time", "23:00"}, {"location", "Tashkent"}, {"latitude", 41.2}, {"longitude", 69.2}, {"timezone_offset", 5}, {"method", "uzbekistan"}, {"asr", "hanafi"}, {"auto_location", true}, {"last_location_provider", QJsonValue::Null}, {"last_location_sync", QJsonValue::Null}});
            else daemon->respond(socket, request, QJsonObject{});
        };
    }


    // A daemon serving the rich tracking fixture. Mutations are recorded in
    // FakeDaemon::requests and answered with an empty canonical result (or
    // the saved row for entity adds), so the app refetches.
    struct TrackingState {
        QJsonObject day;
        QJsonObject status;
        QJsonArray backlog;
        QJsonArray resources;
        QJsonArray goals;
        QJsonArray goalHistory;
        qint64 revision = 100;
    };
    static QJsonObject resourceRow(const QString &kind, const QString &id, const QJsonValue &item, const QJsonValue &epic,
                                   const QJsonValue &title, const QJsonValue &body, const QJsonValue &url,
                                   const QJsonValue &remind, bool done) {
        return QJsonObject{{"id", id}, {"kind", kind}, {"item_id", item.isUndefined() ? QJsonValue(QJsonValue::Null) : item},
                          {"epic_id", epic.isUndefined() ? QJsonValue(QJsonValue::Null) : epic},
                          {"title", title.isUndefined() ? QJsonValue(QJsonValue::Null) : title},
                          {"body", body.isUndefined() ? QJsonValue(QJsonValue::Null) : body},
                          {"url", url.isUndefined() ? QJsonValue(QJsonValue::Null) : url},
                          {"remind_at", remind.isUndefined() ? QJsonValue(QJsonValue::Null) : remind},
                          {"done", done}, {"position", 0}, {"version", 1}, {"created_at", "2031-05-14T04:30:00Z"},
                          {"updated_at", "2031-05-14T04:30:00Z"}, {"deleted_at", QJsonValue(QJsonValue::Null)}};
    }
    static QJsonObject rpcFixture(const QString &name) {
        return readJson(QStringLiteral(TEST_SOURCE_DIR "/../../crates/salah-cli/tests/fixtures/rpc/") + name + ".json").object();
    }
    static QJsonObject emptyChanged() {
        return QJsonObject{{"items", QJsonArray{}}, {"sessions", QJsonArray{}}, {"projects", QJsonArray{}},
                           {"epics", QJsonArray{}}, {"tags", QJsonArray{}}, {"metrics", QJsonArray{}}, {"marks", QJsonArray{}},
                           {"resources", QJsonArray{}}, {"goals", QJsonArray{}}, {"deleted", QJsonArray{}}};
    }
    static FakeDaemon::Handler trackingHandler(std::shared_ptr<TrackingState> state) {
        return [state](const QJsonObject &request, QLocalSocket *socket, FakeDaemon *server) {
            const QString method = request.value("method").toString();
            const QJsonObject params = request.value("params").toObject();
            if (method == "day.get") server->respond(socket, request, state->day);
            else if (method == "status.get") server->respond(socket, request, state->status);
            else if (method == "stats.get") {
                int days = params.value("days").toInt(28);
                if (params.contains("from"))
                    days = int(QDate::fromString(params.value("from").toString(), Qt::ISODate)
                                   .daysTo(QDate::fromString(params.value("to").toString(), Qt::ISODate))) + 1;
                const QString narrow = params.value("project_id").toString().isEmpty() ? params.value("item_id").toString()
                                                                                         : params.value("project_id").toString();
                server->respond(socket, request, TrackingFixture::stats(params.value("tag_id").toString(), narrow,
                                                                        days > 0 ? days : 28));
            }
            else if (method == "stats.series") {
                if (params.value("interval") == "week")
                    server->respond(socket, request, fixture("stats_series_split_item"));
                else
                    server->respond(socket, request, TrackingFixture::trendMonth(params.value("measures").toArray(),
                                                                                  params.value("split").toString()));
            }
            else if (method == "stats.table") {
                QJsonObject table = TrackingFixture::statsTable(params.value("group").toString("week"));
                table["filter"] = QJsonObject{{"tag_id", params.value("tag_id")}, {"project_id", params.value("project_id")},
                                              {"epic_id", params.value("epic_id")}, {"item_id", params.value("item_id")}};
                server->respond(socket, request, table);
            }
            else if (method == "sessions.list") {
                const bool filtered = params.contains("values") || params.contains("q") || params.contains("minutes_min");
                if (params.contains("before")) {
                    const QJsonObject older = TrackingFixture::listRow(
                        "l-6", "2031-05-09", "quran", "Read Quran", "quran", "2031-05-09T00:40:00Z", 20,
                        QJsonObject{{"focus", 3.0}}, "Juz 12 started");
                    server->respond(socket, request, QJsonObject{{"rows", QJsonArray{older}}, {"next", QJsonValue::Null}});
                } else {
                    QJsonObject list = TrackingFixture::sessionsList(filtered, !filtered);
                    if (params.contains("epic_id")) {
                        QJsonArray rows;
                        for (const QJsonValue &row : list.value("rows").toArray())
                            if (row.toObject().value("epic_id").toString() == params.value("epic_id").toString()) rows.append(row);
                        list = QJsonObject{{"rows", rows}, {"next", QJsonValue::Null}};
                    }
                    server->respond(socket, request, list);
                }
            }
            else if (method == "items.list") {
                QJsonArray rows;
                for (const QJsonValue &block : state->day.value("blocks").toArray())
                    for (const QJsonValue &value : block.toObject().value("items").toArray()) {
                        QJsonObject row = value.toObject();
                        row["project_id"] = row.take("project").toObject().value("id");
                        rows.append(row);
                    }
                for (const QJsonValue &row : state->backlog)
                    rows.append(row);
                server->respond(socket, request, rows);
            }
            else if (method == "config.get") server->respond(socket, request, QJsonObject{{"sleep_time", "23:00"}, {"location", "Tashkent"}, {"latitude", 41.2}, {"longitude", 69.2}, {"timezone_offset", 5}, {"method", "uzbekistan"}, {"method_name", "Muslim Board of Uzbekistan"}, {"asr", "hanafi"}, {"auto_location", true}, {"last_location_provider", QJsonValue::Null}, {"last_location_sync", QJsonValue::Null}});
            else if (method == "projects.list") server->respond(socket, request, TrackingFixture::projects());
            else if (method == "epics.list") {
                QJsonArray rows;
                for (const QJsonValue &row : TrackingFixture::epics())
                    if (!params.contains("project_id") || row.toObject().value("project_id") == params.value("project_id")) rows.append(row);
                server->respond(socket, request, rows);
            }
            else if (method == "backlog.list") {
                QJsonArray rows;
                const QString project = params.value("project_id").toString();
                const QString tag = params.value("tag_id").toString();
                for (const QJsonValue &row : state->backlog) {
                    const QJsonObject task = row.toObject();
                    const QString owner = task.value("project_id").toString();
                    const QJsonArray tags = task.value("tag_ids").toArray();
                    const bool matchesProject = project.isEmpty() || (project == "none" ? owner.isEmpty() : owner == project);
                    const bool matchesTag = tag.isEmpty() || (tag == "none" ? tags.isEmpty() : tags.contains(tag));
                    if (matchesProject && matchesTag)
                        rows.append(row);
                }
                server->respond(socket, request, rows);
            }
            else if (method == "resources.list") {
                QJsonArray rows;
                for (const QJsonValue &row : state->resources) {
                    const QJsonObject r = row.toObject();
                    if (params.contains("item_id") ? r.value("item_id") == params.value("item_id")
                                                   : r.value("epic_id") == params.value("epic_id"))
                        rows.append(row);
                }
                server->respond(socket, request, rows);
            }
            else if (method == "goals.list") server->respond(socket, request, state->goals);
            else if (method == "goals.history") server->respond(socket, request, state->goalHistory);
            else if (method == "tags.list") server->respond(socket, request, TrackingFixture::tags());
            else if (method == "metrics.list") server->respond(socket, request, TrackingFixture::metrics());
            else if (method == "item.preview") server->respond(socket, request, fixture("item_preview"));
            else if (params.contains("op_id")) {
                QJsonObject changed = emptyChanged();
                if (method == "project.add" || method == "project.edit")
                    changed["projects"] = QJsonArray{QJsonObject{{"id", params.value("id").toString("new-project")},
                                                                 {"name", params.value("name")}}};
                else if (method == "epic.add" || method == "epic.edit")
                    changed["epics"] = QJsonArray{QJsonObject{{"id", params.value("id").toString("new-epic")},
                                                              {"name", params.value("name")}}};
                else if (method == "resource.add")
                    changed["resources"] = QJsonArray{resourceRow(params.value("kind").toString(), "new-resource",
                                                                  params.value("item_id"), params.value("epic_id"),
                                                                  params.value("title"), params.value("body"),
                                                                  params.value("url"), params.value("remind_at"), false)};
                else if (method == "resource.edit") {
                    for (const QJsonValue &row : state->resources)
                        if (row.toObject().value("id") == params.value("id")) {
                            QJsonObject edited = row.toObject();
                            for (const QString &key : {"title", "body", "url", "remind_at", "done"})
                                if (params.contains(key)) edited[key] = params.value(key);
                            changed["resources"] = QJsonArray{edited};
                        }
                }
                else if (method == "resource.remove")
                    changed["deleted"] = QJsonArray{params.value("id")};
                else if (method == "goal.edit") {
                    for (int i = 0; i < state->goals.size(); ++i) {
                        auto row = state->goals.at(i).toObject();
                        if (row.value("id") != params.value("id")) continue;
                        row["replaces_id"] = row.value("id");
                        row["id"] = row.value("id").toString() + "-successor";
                        row["target"] = params.value("target");
                        state->goals[i] = row;
                        changed["goals"] = QJsonArray{row};
                        break;
                    }
                }
                else if (method == "goal.add" || method == "goal.end")
                    changed["goals"] = QJsonArray{QJsonObject{{"id", params.value("id").toString("new-goal")}}};
                else if (method == "goal.remove")
                    changed["deleted"] = QJsonArray{params.value("id")};
                else if (method == "epic.remove")
                    changed["deleted"] = QJsonArray{params.value("id")};
                else if (method == "tag.add" || method == "tag.edit")
                    changed["tags"] = QJsonArray{QJsonObject{{"id", params.value("id").toString("new-tag")}}};
                else if (method == "metric.add" || method == "metric.edit")
                    changed["metrics"] = QJsonArray{QJsonObject{{"id", params.value("id").toString("new-metric")}}};
                else if (method == "session.remove" || method == "tag.remove")
                    changed["deleted"] = QJsonArray{params.value("id")};
                server->respond(socket, request, QJsonObject{{"revision", ++state->revision},
                                                             {"op_id", params.value("op_id")},
                                                             {"changed", changed}});
            }
            else server->respond(socket, request, QJsonObject{});
        };
    }
    static std::shared_ptr<TrackingState> trackingState(bool withRunning = true) {
        auto state = std::make_shared<TrackingState>();
        state->day = TrackingFixture::day(fixture("day"), withRunning);
        state->status = TrackingFixture::status(fixture("status_running"), withRunning);
        return state;
    }
    // Saves a PNG of the window when SALAH_UI_SHOT_DIR is set; always checks
    // that the window renders.
    static void shotWindow(QQuickWindow *window, const QString &name) {
        QTest::qWait(250);
        const QImage image = window->grabWindow();
        QVERIFY2(!image.isNull(), qPrintable(name));
        const QString dir = qEnvironmentVariable("SALAH_UI_SHOT_DIR");
        if (dir.isEmpty()) return;
        QDir().mkpath(dir);
        QVERIFY2(image.save(dir + "/" + name), qPrintable(name));
    }
    static void trendsScreens(QQuickWindow *window, QObject *stats) {
        auto *chart = qobject_cast<QQuickItem *>(findByName(stats, "trendsChart"));
        auto *flick = stats->property("contentItem").value<QQuickItem *>();
        QVERIFY(chart && flick);
        auto click = [&](const QString &name) {
            QObject *button = findItemByName(chart, name);
            return button && QMetaObject::invokeMethod(button, "clicked");
        };
        auto placeChart = [&] {
            const qreal top = chart->mapToItem(flick, QPointF()).y();
            flick->setProperty("contentY", flick->property("contentY").toReal() + top - 10);
        };
        QVERIFY(click("trendsMeasure_focus"));
        QVERIFY(click("trendsMeasure_minutes"));
        QVERIFY(click("trendsSplit_item"));
        QTRY_COMPARE(chart->property("shown").toList().size(), 3);
        QCOMPARE(chart->property("periods").toList().size(), 28);
        QCOMPARE(chart->property("panels").toList().size(), 1);
        QTest::qWait(50);
        placeChart();
        shotWindow(window, "trends-focus-tasks.png");
        QVERIFY(click("trendsLine_1"));
        QCOMPARE(chart->property("shown").toList().size(), 2);
        QVERIFY(click("trendsLine_1"));

        QVERIFY(click("trendsSplit_"));
        QVERIFY(click("trendsMeasure_minutes"));
        QTRY_COMPARE(chart->property("panels").toList().size(), 2);
        placeChart();
        shotWindow(window, "trends-mixed-split.png");
        QVERIFY(click("trendsRelative"));
        QCOMPARE(chart->property("panels").toList().size(), 1);
        auto *hover = qobject_cast<QQuickItem *>(findItemByName(chart, "trendsHover"));
        QVERIFY(hover);
        // The second period has Focus 3.2 and 32 minutes across two sessions.
        QTest::mouseMove(window, hover->mapToScene({hover->width() / 27, 95}).toPoint());
        QObject *tip = findByName(window, "statsTooltip");
        QVERIFY(tip);
        QTRY_VERIFY(tip->property("visible").toBool());
        const QString tooltip = tip->property("text").toString();
        QVERIFY2(tooltip.contains("32 min") && tooltip.contains("3.2"), qPrintable(tooltip));
        QCOMPARE(tooltip.count("2 sessions"), 2);
        shotWindow(window, "trends-mixed-relative.png");
        QTest::mouseMove(window, QPoint(10, 10));
        QVERIFY(click("chartTableButton"));
        const auto rows = chart->property("tableRows").toList();
        QCOMPARE(rows.size(), 28);
        QVERIFY(rows[5].toList().contains("—"));
        QVERIFY(rows[5].toList().contains("0 min"));
        QVERIFY(click("chartChartButton"));
        QVERIFY(click("trendsSplitView"));
        QVERIFY(click("trendsMeasure_focus"));
        flick->setProperty("contentY", 0);
    }
    static QObject *findItemByName(QObject *root, const QString &name) {
        auto *item = qobject_cast<QQuickItem *>(root);
        if (!item) return nullptr;
        for (QQuickItem *child : quickDescendants(item))
            if (child->objectName() == name) return child;
        return nullptr;
    }
    // Visual search: delegates of a Repeater are not QObject children of the
    // items above them, so findChildren cannot see them.
    static QList<QObject *> visualNamed(QQuickWindow *window, const QString &name) {
        QList<QObject *> found;
        for (QQuickItem *item : quickDescendants(window->contentItem()))
            if (item->objectName() == name) found.append(item);
        return found;
    }
    // The first visible item with this name (hidden pages keep their own copies).
    static QObject *firstShown(QQuickWindow *window, const QString &name) {
        for (QObject *object : visualNamed(window, name))
            if (auto *item = qobject_cast<QQuickItem *>(object); item && item->isVisible()) return object;
        return nullptr;
    }
    static bool hasVisualText(QQuickWindow *window, const QString &text) {
        for (QQuickItem *item : quickDescendants(window->contentItem()))
            if (item->isVisible() && item->property("text").toString() == text) return true;
        return false;
    }
    static QObject *findByName(QObject *root, const QString &name) {
        for (QObject *object : root->findChildren<QObject *>())
            if (object->objectName() == name) return object;
        return nullptr;
    }
    static QString daemon_path(const QTemporaryDir &dir) { return dir.filePath("salah.sock"); }
    // Create a Unix socket inode with no listener: connecting to it is refused,
    // exactly like a daemon that exited without cleaning up its socket file.
    static bool createStaleSocket(const QString &path) {
        const int fd = ::socket(AF_UNIX, SOCK_STREAM, 0);
        if (fd < 0) return false;
        sockaddr_un address{};
        address.sun_family = AF_UNIX;
        const QByteArray encoded = QFile::encodeName(path);
        if (encoded.size() >= int(sizeof(address.sun_path))) { ::close(fd); return false; }
        std::memcpy(address.sun_path, encoded.constData(), size_t(encoded.size()) + 1);
        QLocalServer::removeServer(path);
        const bool bound = ::bind(fd, reinterpret_cast<sockaddr *>(&address), sizeof(address)) == 0;
        ::close(fd);
        return bound;
    }

    QTemporaryDir *m_runtime = nullptr;
    QTemporaryDir m_settingsDir;
    bool m_integrationEnvironment = false;

private slots:
    void initTestCase() {
        // The app runs with the Material style (main.cpp); render the same.
        QQuickStyle::setStyle("Material");
        // The opt-in integration tests below intentionally use the caller's
        // desktop environment; everything else runs in the sandbox set up in
        // init().
        // UI preferences (Statistics tab, table columns) never touch the
        // user's own settings file.
        QVERIFY(m_settingsDir.isValid());
        QSettings::setPath(QSettings::NativeFormat, QSettings::UserScope, m_settingsDir.path());
        QSettings::setPath(QSettings::IniFormat, QSettings::UserScope, m_settingsDir.path());
        m_integrationEnvironment = !qEnvironmentVariableIsEmpty("SALAH_PROTOCOL_SMOKE") ||
                                   !qEnvironmentVariableIsEmpty("SALAH_APP_SMOKE") ||
                                   !qEnvironmentVariableIsEmpty("A9_SCREENSHOT_DIR");
    }
    void init() {
        QSettings(QStringLiteral("salah"), QStringLiteral("salah-app")).clear();
        if (m_integrationEnvironment) return;
        delete m_runtime;
        m_runtime = new QTemporaryDir;
        QVERIFY(m_runtime->isValid());
        // Never inherit the desktop session's daemon socket or CLI binary.
        qputenv("XDG_RUNTIME_DIR", m_runtime->path().toUtf8());
        qunsetenv("SALAH_SOCKET");
        qputenv("SALAH_BIN", m_runtime->filePath("salah").toUtf8());
    }
    void cleanup() {
        delete m_runtime;
        m_runtime = nullptr;
    }

    void socketHandshakeSubscribesAndCorrelatesRequests() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        daemon.setHandler(readHandler(fixture("day")));
        CoreClient client(nullptr, QString(), 2000, daemon_path(dir));
        QSignalSpy spy(&client, &CoreClient::requestFinished);
        QTRY_VERIFY_WITH_TIMEOUT(!client.offline(), 3000);
        CoreClient::Result dayResult, configResult;
        client.day(QDate(2026,11,22), [&](const CoreClient::Result &result) { dayResult = result; });
        client.configShow([&](const CoreClient::Result &result) { configResult = result; });
        QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 2, 3000);
        QVERIFY2(dayResult.ok, qPrintable(dayResult.error.message)); QVERIFY(std::holds_alternative<Dto::DayPlan>(dayResult.value));
        QVERIFY(configResult.ok); QVERIFY(std::holds_alternative<Dto::Config>(configResult.value));
        QCOMPARE(daemon.hellos, 1); QCOMPARE(daemon.subscribes, 1);
        QCOMPARE(daemon.requests.at(0).value("method").toString(), QString("hello"));
        QCOMPARE(daemon.requests.at(0).value("params").toObject().value("protocol").toArray().first().toInt(), 1);
        QCOMPARE(daemon.requests.at(0).value("params").toObject().value("actor").toString(), QString("app"));
        QCOMPARE(daemon.requests.at(1).value("method").toString(), QString("subscribe"));
        QCOMPARE(daemon.requests.at(1).value("params").toObject().value("topics").toArray(),
                 (QJsonArray{"changes", "clock"}));
        QSet<qint64> ids;
        for (const QJsonObject &request : std::as_const(daemon.requests)) ids.insert(request.value("id").toVariant().toLongLong());
        QCOMPARE(ids.size(), daemon.requests.size());
    }
    void socketRequestTimesOut() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        daemon.setHandler([](const QJsonObject &, QLocalSocket *, FakeDaemon *) {});
        CoreClient client(nullptr, QString(), 120, daemon_path(dir));
        QSignalSpy spy(&client, &CoreClient::requestFinished);
        CoreClient::Result result;
        QTRY_VERIFY_WITH_TIMEOUT(!client.offline(), 3000);
        client.parse("slow", [&](const CoreClient::Result &reply) { result = reply; });
        QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 1, 3000);
        QVERIFY(!result.ok); QCOMPARE(result.error.kind, CoreClient::ErrorKind::Timeout);
        QVERIFY(result.error.message.contains("timed out"));
    }
    void socketReconnectsAfterDrop() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        daemon.setHandler(readHandler(fixture("day")));
        CoreClient client(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy spy(&client, &CoreClient::requestFinished);
        CoreClient::Result first;
        QTRY_VERIFY_WITH_TIMEOUT(!client.offline(), 3000);
        client.configShow([&](const CoreClient::Result &result) { first = result; });
        QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 1, 3000); QVERIFY(first.ok);
        daemon.dropClients();
        QTest::qWait(200);
        CoreClient::Result second;
        client.configShow([&](const CoreClient::Result &result) { second = result; });
        QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 2, 6000);
        QVERIFY(second.ok);
        QCOMPARE(daemon.hellos, 2); QCOMPARE(daemon.subscribes, 2);
    }
    void socketConflictReloadsAndShowsServerMessage() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        QJsonObject day = fixture("day");
        QJsonArray blocks = day.value("blocks").toArray();
        for (int b = 0; b < blocks.size(); ++b) {
            QJsonObject block = blocks[b].toObject();
            QJsonArray items = block.value("items").toArray();
            for (int i = 0; i < items.size(); ++i) {
                QJsonObject item = items[i].toObject();
                if (item.value("id").toString() == "<id:4>") item["version"] = 7;
                items[i] = item;
            }
            block["items"] = items;
            blocks[b] = block;
        }
        day["blocks"] = blocks;
        auto reads = readHandler(day);
        int dayRequests = 0, editRequests = 0;
        QJsonObject editParams;
        daemon.setHandler([&, reads](const QJsonObject &request, QLocalSocket *socket, FakeDaemon *server) {
            if (request.value("method").toString() == "item.edit") {
                ++editRequests;
                editParams = request.value("params").toObject();
                server->respondError(socket, request, -32004, "conflict", "item \"<id:4>\" is at version 4, not 3");
                return;
            }
            if (request.value("method").toString() == "day.get") ++dayRequests;
            reads(request, socket, server);
        });
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy spy(&backend, &Backend::requestFinished);
        backend.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 4, 3000);
        const int row = timelineItem(backend.dayTimeline(), "<id:4>");
        QVERIFY(row >= 0);
        QVariantMap item = itemEditMap(backend, "<id:4>");
        item["title"] = "Changed";
        backend.saveItem(item);
        QTRY_VERIFY_WITH_TIMEOUT(editRequests == 1, 3000);
        QCOMPARE(editParams.value("expected_version").toInt(), 7);
        QTRY_VERIFY_WITH_TIMEOUT(backend.error().contains("version 4, not 3"), 3000);
        QTRY_VERIFY_WITH_TIMEOUT(dayRequests >= 2, 3000);
    }
    void changedNotificationReloadsOncePerRevision() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        daemon.setHandler(readHandler(fixture("day")));
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy spy(&backend, &Backend::requestFinished);
        backend.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 4, 3000);
        auto dayRequests = [&] {
            int count = 0;
            for (const QJsonObject &request : std::as_const(daemon.requests))
                if (request.value("method").toString() == "day.get") ++count;
            return count;
        };
        const int base = dayRequests();
        daemon.notifyChanged(11);
        daemon.notifyChanged(11);
        QTRY_VERIFY_WITH_TIMEOUT(dayRequests() >= base + 1, 3000);
        QTest::qWait(300);
        QCOMPARE(dayRequests(), base + 1);
        daemon.notifyChanged(12);
        QTRY_COMPARE_WITH_TIMEOUT(dayRequests(), base + 2, 3000);
    }
    void socketMutationAppliesCanonicalRows() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        QJsonObject currentDay = fixture("day");
        daemon.setHandler([currentDay](const QJsonObject &request, QLocalSocket *socket, FakeDaemon *server) mutable {
            const QString method = request.value("method").toString();
            if (request.value("method").toString() == "item.add") {
                const QJsonObject params = request.value("params").toObject();
                QJsonObject row = fixture("item_add").value("changed").toObject().value("items").toArray().first().toObject();
                row["id"] = "new-1"; row["title"] = params.value("title").toString(); row["version"] = 2;
                QJsonObject changed = fixture("item_add").value("changed").toObject();
                changed["items"] = QJsonArray{row};
                QJsonArray blocks = currentDay.value("blocks").toArray();
                for (int i = 0; i < blocks.size(); ++i) {
                    QJsonObject block = blocks[i].toObject();
                    if (block.value("start").toString() != "fajr") continue;
                    QJsonArray rows = block.value("items").toArray();
                    QJsonObject dayRow = rows.first().toObject();
                    dayRow["id"] = "new-1";
                    dayRow["title"] = params.value("title").toString();
                    dayRow["done"] = false;
                    dayRow["actual_minutes"] = 0;
                    dayRow["sessions"] = QJsonArray{};
                    dayRow["position"] = rows.size();
                    dayRow["version"] = 2;
                    rows.append(dayRow); block["items"] = rows; blocks[i] = block;
                    break;
                }
                currentDay["blocks"] = blocks;
                server->respond(socket, request, QJsonObject{{"revision", 101}, {"op_id", params.value("op_id")}, {"changed", changed}});
                return;
            }
            if (method == "day.get") server->respond(socket, request, currentDay);
            else if (method == "stats.get") server->respond(socket, request, fixture("stats"));
            else if (method == "config.get") server->respond(socket, request, QJsonObject{{"sleep_time", "23:00"}, {"location", "Tashkent"}, {"latitude", 41.2}, {"longitude", 69.2}, {"timezone_offset", 5}, {"method", "uzbekistan"}, {"asr", "hanafi"}, {"auto_location", true}, {"last_location_provider", QJsonValue::Null}, {"last_location_sync", QJsonValue::Null}});
            else server->respond(socket, request, QJsonObject{});
        });
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy spy(&backend, &Backend::requestFinished);
        backend.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 4, 3000);
        backend.addQuick("Canonical", "fajr");
        auto contains = [&](const QString &id) { return timelineItem(backend.dayTimeline(), id) >= 0; };
        QTRY_VERIFY_WITH_TIMEOUT(contains("new-1"), 3000);
        for (int row = 0; row < backend.dayTimeline()->rowCount(); ++row)
            QVERIFY(!backend.dayTimeline()->data(backend.dayTimeline()->index(row, 0),
                    DayTimelineModel::ItemIdRole).toString().startsWith("pending-"));
    }
    void missingSocketFallsBackToCliAndReportsOffline() {
        QTemporaryDir dir;
        const QString log = dir.filePath("cli-args");
        const QString script = dir.filePath("salah");
        const QString fixturePath = dir.filePath("config.json");
        write(fixturePath, R"JSON({"asr":"hanafi","auto_location":true,"last_location_provider":null,"last_location_sync":null,"latitude":41.2,"location":"Tashkent","longitude":69.2,"method":"uzbekistan","sleep_time":"23:00","timezone_offset":5})JSON");
        write(script, "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '" + log.toUtf8() + "'\ncat '" + fixturePath.toUtf8() + "'\n");
        QFile::setPermissions(script, QFile::ReadOwner | QFile::WriteOwner | QFile::ExeOwner);
        CoreClient client(nullptr, script, 2000, dir.filePath("missing/salah.sock"));
        QSignalSpy spy(&client, &CoreClient::requestFinished);
        QTRY_VERIFY_WITH_TIMEOUT(client.offline(), 3000);
        CoreClient::Result result;
        client.configShow([&](const CoreClient::Result &reply) { result = reply; });
        QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 1, 3000);
        QVERIFY(result.ok);
        QFile file(log); QVERIFY(file.open(QIODevice::ReadOnly));
        QCOMPARE(QString::fromUtf8(file.readAll()).trimmed(), QString("config show"));
        QVERIFY(client.offline());
    }

    void refusingSocketFallsBackToCliImmediately() {
        QTemporaryDir dir;
        const QString socket = dir.filePath("stale.sock");
        if (!createStaleSocket(socket)) QSKIP("AF_UNIX unavailable in sandbox");
        QVERIFY(QFileInfo::exists(socket));

        const QString log = dir.filePath("cli-args");
        const QString script = dir.filePath("salah");
        const QString fixturePath = dir.filePath("config.json");
        write(fixturePath, R"JSON({"asr":"hanafi","auto_location":true,"last_location_provider":null,"last_location_sync":null,"latitude":41.2,"location":"Tashkent","longitude":69.2,"method":"uzbekistan","sleep_time":"23:00","timezone_offset":5})JSON");
        write(script, "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '" + log.toUtf8() + "'\ncat '" + fixturePath.toUtf8() + "'\n");
        QFile::setPermissions(script, QFile::ReadOwner | QFile::WriteOwner | QFile::ExeOwner);
        CoreClient client(nullptr, script, 2000, socket);
        QSignalSpy spy(&client, &CoreClient::requestFinished);
        CoreClient::Result result;
        QElapsedTimer elapsed;
        elapsed.start();
        client.configShow([&](const CoreClient::Result &reply) { result = reply; });
        QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 1, 3000);
        QVERIFY2(result.ok, qPrintable(result.error.message));
        QVERIFY(client.offline());
        // No activation retry/delay when the socket refuses the connection.
        QVERIFY2(elapsed.elapsed() < 1000, qPrintable(QString::number(elapsed.elapsed())));
        QFile file(log); QVERIFY(file.open(QIODevice::ReadOnly));
        QCOMPARE(QString::fromUtf8(file.readAll()).trimmed(), QString("config show"));
    }

    void failedCliResultIsReportedWithoutCrashing() {
        QTemporaryDir dir;
        const QString script = dir.filePath("salah");
        write(script, "#!/bin/sh\nprintf 'salah: no item \\\"id-1\\\"\\n' >&2\nexit 7\n");
        QFile::setPermissions(script, QFile::ReadOwner | QFile::WriteOwner | QFile::ExeOwner);
        CoreClient client(nullptr, script, 2000, dir.filePath("missing.sock"));
        QSignalSpy spy(&client, &CoreClient::requestFinished);
        CoreClient::Result result;
        client.sessionStop("id-1", {}, {}, [&](const CoreClient::Result &reply) { result = reply; });
        QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 1, 3000);
        QVERIFY(!result.ok);
        QCOMPARE(result.error.kind, CoreClient::ErrorKind::Exit);
        QCOMPARE(result.error.message, QString("no item \"id-1\""));
        QVERIFY(std::holds_alternative<std::monostate>(result.value));
    }

    void protocolSmoke() {
        if (qEnvironmentVariableIsEmpty("SALAH_PROTOCOL_SMOKE")) return;
        Backend backend;
        Theme theme;
        QSignalSpy spy(&backend, &Backend::requestFinished);
        backend.reload();
        QTRY_VERIFY_WITH_TIMEOUT(spy.size() >= 3, 8000);
        QVERIFY2(backend.error().isEmpty(), qPrintable(backend.error()));
        QVERIFY(!backend.offlineMode());
        QQmlEngine engine;
        engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl::fromLocalFile(QStringLiteral(TEST_SOURCE_DIR "/../src/qml/Main.qml")));
        QScopedPointer<QObject> root(component.create());
        QVERIFY2(root, qPrintable(component.errorString()));
        root->setProperty("visible", false);
        auto findItem = [&](const QString &title) { return timelineTitle(backend.dayTimeline(), title); };
        backend.addQuick("Smoke add", "fajr");
        auto canonical = [&](const QString &title) {
            const int row = findItem(title);
            if (row < 0) return false;
            const auto id = backend.dayTimeline()->data(backend.dayTimeline()->index(row, 0),
                                                        DayTimelineModel::ItemIdRole).toString();
            const bool pending = backend.dayTimeline()->data(backend.dayTimeline()->index(row, 0),
                                                             DayTimelineModel::PendingRole).toBool();
            return !id.startsWith("pending-") && !pending;
        };
        QTRY_VERIFY_WITH_TIMEOUT(canonical("Smoke add"), 8000);
        const int added = findItem("Smoke add");
        const QString id = backend.dayTimeline()->data(backend.dayTimeline()->index(added, 0),
                                                       DayTimelineModel::ItemIdRole).toString();
        backend.moveItem(id, "dhuhr", {}, {});
        QTRY_VERIFY_WITH_TIMEOUT([&] {
            const int row = timelineItem(backend.dayTimeline(), id);
            return row >= 0 && backend.dayTimeline()->data(backend.dayTimeline()->index(row, 0),
                    DayTimelineModel::BlockRole).toString() == "dhuhr";
        }(), 8000);
        auto started = [&] {
            const int row = findItem("Smoke add");
            return row >= 0 && !backend.dayTimeline()->data(backend.dayTimeline()->index(row, 0),
                    DayTimelineModel::TimerStartedAtRole).toString().isEmpty();
        };
        backend.toggleTimer(id, false);
        QTRY_VERIFY_WITH_TIMEOUT(started(), 8000);
        backend.toggleTimer(id, true);
        QTRY_VERIFY_WITH_TIMEOUT(!started(), 8000);
        const int prayer = prayerRow(backend.prayers(), "fajr");
        QVERIFY(prayer >= 0);
        bool marked = backend.prayers()->data(backend.prayers()->index(prayer, 0),
                                             PrayerModel::MarkedRole).toBool();
        backend.togglePrayer("fajr", marked);
        QTRY_VERIFY_WITH_TIMEOUT([&] {
            const int row = prayerRow(backend.prayers(), "fajr");
            return row >= 0 && backend.prayers()->data(backend.prayers()->index(row, 0),
                    PrayerModel::MarkedRole).toBool() != marked;
        }(), 8000);
        QProcess external;
        external.start(qEnvironmentVariable("SALAH_BIN", "salah"), {"item", "add", "--json", "--", "External smoke"});
        QVERIFY(external.waitForFinished(8000));
        QCOMPARE(external.exitCode(), 0);
        QTRY_VERIFY_WITH_TIMEOUT(findItem("External smoke") >= 0, 8000);
        QVERIFY2(backend.error().isEmpty(), qPrintable(backend.error()));
        root->setProperty("visible", false);
    }
    void decodersAcceptGoldenFixtures() {
        QVERIFY(Dto::decode<Dto::DayPlan>(fixture("day")));
        QVERIFY(Dto::decode<Dto::Stats>(fixture("stats")));
        QVERIFY(Dto::decode<Dto::ParseDraft>(fixture("item_parse")));
        for (const QString &name : {"item_add", "item_edit", "item_move", "item_remove",
                                    "session_start", "session_log", "session_stop", "prayer_mark",
                                    "project_add", "tag_add", "metric_add"})
            QVERIFY2(Dto::decode<Dto::MutationResult>(fixture(name)), qPrintable(name));
    }
    void decoderRejectsBadFieldsAndAllowsExtras() {
        QJsonObject day=fixture("day");
        day["future_v1_field"] = QJsonObject{{"anything",true}};
        QVERIFY(Dto::decode<Dto::DayPlan>(day));
        QJsonObject missing=day; missing.remove("blocks"); auto a=Dto::decode<Dto::DayPlan>(missing);
        QVERIFY(!a); QCOMPARE(a.error->formatted(),QString("day.blocks: missing required field"));
        QJsonObject mistyped=day; mistyped["open"]="zero"; auto b=Dto::decode<Dto::DayPlan>(mistyped);
        QVERIFY(!b); QCOMPARE(b.error->formatted(),QString("day.open: expected number"));
        QJsonArray blocks=day["blocks"].toArray(); int blockIndex = -1; QJsonObject block; QJsonArray items;
        for (int i = 0; i < blocks.size(); ++i) { block = blocks[i].toObject(); items = block["items"].toArray(); if (!items.isEmpty()) { blockIndex = i; break; } }
        QVERIFY(blockIndex >= 0); QJsonObject item=items[0].toObject(); item["done"]="future"; items[0]=item; block["items"]=items; blocks[blockIndex]=block; day["blocks"]=blocks;
        auto c=Dto::decode<Dto::DayPlan>(day); QVERIFY(!c);
        QCOMPARE(c.error->formatted(),QString("day.blocks[%1].items[0].done: expected boolean").arg(blockIndex));
    }
    void trackingSurfacesFollowCanonicalFixtures() {
        const QString rpc = QStringLiteral(TEST_SOURCE_DIR "/../../crates/salah-cli/tests/fixtures/rpc/");
        const QJsonObject invalid = readJson(rpc + "session_stop_invalid_value.json").object()
                                        .value("error").toObject().value("data").toObject();
        QCOMPARE(invalid.value("field").toString(), QString("values"));

        const QJsonArray projects = readJson(rpc + "projects_list.json").object().value("result").toArray();
        const QJsonObject binding = projects.first().toObject().value("metrics").toArray().first().toObject();
        QVERIFY(binding.value("metric").isObject());
        QVERIFY(binding.contains("required"));

        const QJsonObject stats = fixture("stats");
        QCOMPARE(stats.value("series").toArray().first().toObject().value("metric_id").toString(), QString("minutes"));
        QVERIFY(stats.value("projects").isArray());
        QVERIFY(stats.value("blocks").isArray());

        // The bar widget's own sources are checked by plugin/local.salah/tests;
        // the app suite does not pin the plugin's internals.

        QFile settings(QStringLiteral(TEST_SOURCE_DIR "/../src/backend.cpp"));
        QVERIFY(settings.open(QIODevice::ReadOnly));
        const QByteArray settingsSource = settings.readAll();
        QVERIFY(settingsSource.contains("void Backend::saveEntity"));
        QVERIFY(settingsSource.contains("QStringLiteral(\".edit\")"));
        QVERIFY(settingsSource.contains("QStringLiteral(\".add\")"));
    }
    void coreClientDecodesSessionStopMutation() {
        QTemporaryDir dir; const QString path=dir.filePath("salah");
        const QString fixture=QStringLiteral(TEST_SOURCE_DIR "/../../crates/salah-cli/tests/fixtures/rpc/session_stop.json");
        write(path,"#!/bin/sh\ncat >/dev/null\njq -c '{jsonrpc,id:2,result}' '"+fixture.toUtf8()+"'\n"); QFile::setPermissions(path,QFile::ReadOwner|QFile::WriteOwner|QFile::ExeOwner);
        CoreClient client(nullptr,path,2000,dir.filePath("missing.sock"));
        QSignalSpy spy(&client,&CoreClient::requestFinished); CoreClient::Result result;
        client.sessionStop("id-1",{}, {}, [&](const CoreClient::Result&r){result=r;}); QTRY_COMPARE(spy.size(),1);
        QVERIFY2(result.ok, qPrintable(result.error.message)); QVERIFY(std::holds_alternative<Dto::MutationResult>(result.value));
    }
    void coreClientBuildsEveryCommand() {
        QTemporaryDir dir; CoreClient client(nullptr, fakeCore(dir)); QSignalSpy spy(&client, &CoreClient::requestFinished);
        CoreClient::ItemFields add; add.title = "Title -- literal"; add.block = "asr"; add.repeat = "weekly";
        add.weekdays = {"mon", "wed"}; add.projectId = "project-1"; add.plannedMinutes = 30; add.hasPlannedMinutes = true; add.date = QDate(2026,11,23);
        CoreClient::ItemFields edit = add; edit.title = "New title"; edit.hasPlannedMinutes = false;
        client.day(QDate(2026,11,22), {}); client.stats(QDate(2026,11,1),QDate(2026,11,30),{});
        client.itemAdd(add); client.itemEdit("id-1",edit); client.itemRemove("id-old"); client.itemMove("id-1","isha",-2);
        client.sessionLog("id-1", {}, 15, {{"focus", 4}}, "Chapter finished", QDate(2026,11,23));
        client.sessionLog("id-1", "project-1");
        client.sessionStart({}, "project-1"); client.sessionStop("session-1");
        client.prayerMark("fajr",QDate(2026,11,22)); client.prayerUnmark("fajr",QDate(2026,11,22));
        client.parse("read -- carefully",{}); client.configShow({}); client.configSetSleepTime("22:45");
        client.sessionStop("session-2", QJsonObject{{"focus", 4}}, "Finished chapter", {}, true, 32);
        QTRY_COMPARE_WITH_TIMEOUT(spy.size(),16,5000);
        const QStringList lines = coreLogLines(dir);
        QCOMPARE(lines.size(), 16);
        QCOMPARE(lines.at(0),"<day><--date><2026-11-22><--json>");
        QCOMPARE(lines.at(1),"<stats><--from><2026-11-01><--to><2026-11-30><--json>");
        QCOMPARE(lines.at(12), "<item><parse><--json><--><read -- carefully>");
        QCOMPARE(lines.at(13), "<config><show>");
        const auto writes = offlineRequests(dir);
        QCOMPARE(writes.size(), 12);
        const QStringList expected{"item.add", "item.edit", "item.remove", "item.move", "session.log", "session.log",
                                   "session.start", "session.stop", "prayer.mark", "prayer.unmark", "config.set", "session.stop"};
        QSet<QString> ids;
        for (int i = 0; i < writes.size(); ++i) {
            QCOMPARE(writes[i].value("method").toString(), expected[i]);
            const auto id = writes[i].value("params").toObject().value("op_id").toString();
            QVERIFY(!id.isEmpty());
            QVERIFY(!ids.contains(id));
            ids.insert(id);
        }
        auto params = [&](int i) { return writes[i].value("params").toObject(); };
        QCOMPARE(params(0).value("title"), "Title -- literal");
        QCOMPARE(params(0).value("weekdays").toArray(), (QJsonArray{"mon", "wed"}));
        QCOMPARE(params(0).value("planned_minutes"), 30);
        QCOMPARE(params(1).value("planned_minutes"), QJsonValue(QJsonValue::Null));
        QCOMPARE(params(2).value("id"), "id-old");
        QCOMPARE(params(3).value("block"), "isha");
        QCOMPARE(params(4).value("item_id"), "id-1");
        QCOMPARE(params(4).value("minutes"), 15);
        QCOMPARE(params(4).value("date"), "2026-11-23");
        QCOMPARE(params(4).value("values").toObject().value("focus"), 4);
        QCOMPARE(params(4).value("note"), "Chapter finished");
        QVERIFY(!params(5).contains("minutes"));
        QCOMPARE(params(6).value("project_id"), "project-1");
        QCOMPARE(params(7).value("id"), "session-1");
        QCOMPARE(params(8).value("prayer"), "fajr");
        QCOMPARE(params(9).value("date"), "2026-11-22");
        QCOMPARE(params(10).value("sleep_time"), "22:45");
        QCOMPARE(params(11).value("minutes"), 32);
        QCOMPARE(params(11).value("done"), true);
        QCOMPARE(params(11).value("values").toObject().value("focus"), 4);
        QCOMPARE(params(11).value("note"), "Finished chapter");
    }
    void changedNotificationRefreshesTheVisibleReportAndPicker() {
        QTemporaryDir dir;
        FakeDaemon daemon(daemon_path(dir));
        if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        auto state = trackingState();
        daemon.setHandler(trackingHandler(state));
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        Theme theme(dir.path());
        QQmlEngine engine;
        engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl("qrc:/qml/Main.qml"));
        QScopedPointer<QObject> root(component.create());
        QVERIFY2(root, qPrintable(component.errorString()));
        backend.reload();
        waitForBackendIdle(backend);
        root->setProperty("page", 2);
        QObject *stats = findByName(root.data(), "statsScroll");
        QVERIFY(stats);
        QVERIFY(QMetaObject::invokeMethod(stats, "setFilter", Q_ARG(QVariant, "project"), Q_ARG(QVariant, "study")));
        auto invalidate = [&](int revision, const QString &method, const QString &field, const QString &id) {
            waitForBackendIdle(backend);
            const int reports = requestCount(daemon, method);
            const int lists = requestCount(daemon, "items.list");
            daemon.notifyChanged(revision, {"items", "sessions"});
            QTRY_VERIFY(requestCount(daemon, method) > reports);
            QTRY_VERIFY(requestCount(daemon, "items.list") > lists);
            waitForBackendIdle(backend);
            QCOMPARE(lastRequest(daemon, method).value("params").toObject().value(field).toString(), id);
        };
        invalidate(101, "stats.series", "project_id", "study");
        QVERIFY(QMetaObject::invokeMethod(stats, "setTab", Q_ARG(QVariant, "table")));
        invalidate(102, "stats.table", "project_id", "study");
        auto *table = findByName(root.data(), "generalTable");
        QVERIFY(table);
        QVERIFY(QMetaObject::invokeMethod(table, "setGroup", Q_ARG(QVariant, "project")));
        waitForBackendIdle(backend);
        QVERIFY(QMetaObject::invokeMethod(table, "setGroup", Q_ARG(QVariant, "epic")));
        // The project report is still on screen during the epic read.
        auto *action = findItemByName(table, "epicRowOpen");
        QVERIFY(action && !action->property("enabled").toBool());
        waitForBackendIdle(backend);
        QVERIFY(QMetaObject::invokeMethod(stats, "setTab", Q_ARG(QVariant, "notes")));
        invalidate(103, "sessions.list", "project_id", "study");

        QVERIFY(QMetaObject::invokeMethod(root.data(), "openEpic", Q_ARG(QVariant, "e-grammar")));
        waitForBackendIdle(backend);
        const int series = requestCount(daemon, "stats.series");
        invalidate(104, "sessions.list", "epic_id", "e-grammar");
        QVERIFY(requestCount(daemon, "stats.series") > series);
        QCOMPARE(lastRequest(daemon, "stats.series").value("params").toObject().value("epic_id"), "e-grammar");

        // An epic log uses the current tracking day even if Today is browsing history.
        state->day["day"] = "2031-05-13";
        state->day["is_today"] = false;
        backend.selectDay("2031-05-13");
        waitForBackendIdle(backend);
        QVERIFY(backend.selectedDay() != backend.todayDay());
        auto *epic = findByName(root.data(), "epicPage");
        auto *prompt = findByName(root.data(), "sessionPrompt");
        auto *minutes = findByName(root.data(), "sessionMinutesField");
        QVERIFY(epic && prompt && minutes);
        QVERIFY(QMetaObject::invokeMethod(epic, "logTime"));
        minutes->setProperty("text", "15");
        QVERIFY(QMetaObject::invokeMethod(prompt, "save", Q_ARG(QVariant, false), Q_ARG(QVariant, false)));
        QTRY_COMPARE(requestCount(daemon, "session.log"), 1);
        const auto log = lastRequest(daemon, "session.log").value("params").toObject();
        QCOMPARE(log.value("epic_id"), "e-grammar");
        QVERIFY(!log.contains("date"));
    }

    void countdownUsesDeadlineAndQuickAddShowsLiteralMarkup() {
        QTemporaryDir dir;
        Backend backend(nullptr, fake(dir));
        Theme theme(dir.path());
        QQmlEngine engine;
        engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl("qrc:/qml/Main.qml"));
        QScopedPointer<QObject> root(component.create());
        QVERIFY2(root, qPrintable(component.errorString()));
        waitForBackendIdle(backend);
        auto day = Dto::decode<Dto::DayPlan>(fixture("day"));
        QVERIFY(day && day.value->now);
        auto next = day.value->now->json.value("next").toObject();
        next["at"] = QDateTime::currentDateTimeUtc().addSecs(61).toString(Qt::ISODateWithMs);
        day.value->now->json["next"] = next;
        backend.nowState()->update(*day.value, QDateTime::currentMSecsSinceEpoch());
        root->setProperty("countdown", 400);
        QTRY_VERIFY_WITH_TIMEOUT(root->property("countdown").toInt() <= 61, 2500);
        QVERIFY(root->property("countdown").toInt() >= 57);
        auto *preview = findByName(root.data(), "quickPreview");
        QVERIFY(preview);
        Dto::ParseDraft draft;
        draft.title = "<b>Read</b>";
        backend.parseDraft()->update(draft);
        QCOMPARE(preview->property("textFormat").toInt(), int(Qt::PlainText));
        QVERIFY(preview->property("text").toString().contains("<b>Read</b>"));
    }

    void socketReadsCoalesceOnlyIdenticalParamsAndFinishEveryCaller() {
        QTemporaryDir dir;
        FakeDaemon daemon(daemon_path(dir));
        if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        daemon.setHandler([](const QJsonObject &request, QLocalSocket *socket, FakeDaemon *server) {
            // Slow enough that the first read is still in flight when the
            // later calls are queued or coalesced.
            QTimer::singleShot(400, server, [=] { server->respond(socket, request, request.value("params")); });
        });
        CoreClient client(nullptr, QString(), 4000, daemon_path(dir));
        const QJsonObject a{{"measures", QJsonArray{"minutes"}}, {"epic_id", "a"}};
        QJsonObject b = a, explicitBounds = a;
        b["epic_id"] = "b";
        explicitBounds["values"] = QJsonObject{}; // Same CLI arguments, different RPC params.
        QList<QJsonObject> replies;
        auto call = [&](const QJsonObject &params) {
            client.statsSeries(params, [&](const CoreClient::Result &reply) {
                QVERIFY(reply.ok);
                replies.append(std::get<Dto::Json>(reply.value).json);
            });
        };
        call(a);
        QTRY_COMPARE(requestCount(daemon, "stats.series"), 1);
        call(b);
        call(a);
        call(explicitBounds);
        QTRY_COMPARE(replies.size(), 4);
        QCOMPARE(replies, (QList<QJsonObject>{a, a, b, explicitBounds}));
        QCOMPARE(requestCount(daemon, "stats.series"), 3);
    }

    void sessionEditorCorrelatesRepliesAndKeepsExplicitClears() {
        QTemporaryDir dir;
        Backend backend(nullptr, fake(dir));
        Theme theme(dir.path());
        QQmlEngine engine;
        engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl("qrc:/qml/Main.qml"));
        QScopedPointer<QObject> root(component.create());
        QVERIFY2(root, qPrintable(component.errorString()));
        QObject *notes = findByName(root.data(), "notesList");
        QObject *prompt = findByName(root.data(), "sessionPrompt");
        QObject *note = findByName(root.data(), "sessionNoteField");
        QVERIFY(notes && prompt && note);
        auto submit = [&](const QString &id, int version) {
            QVariantMap row{{"id", id}, {"minutes", 15}, {"values", QVariantMap{{"focus", 3}}},
                            {"version", version}, {"note", "old"}};
            QVERIFY(QMetaObject::invokeMethod(notes, "openSession", Q_ARG(QVariant, row)));
            note->setProperty("text", "");
            prompt->setProperty("values", QVariantMap{});
            QVERIFY(QMetaObject::invokeMethod(prompt, "save", Q_ARG(QVariant, false), Q_ARG(QVariant, false)));
        };
        submit("first", 7);
        const QString first = prompt->property("operation").toString();
        QVERIFY(!first.isEmpty());
        root->setProperty("sessionPromptVisible", false);
        submit("conflicting-edit", 4);
        const QString second = prompt->property("operation").toString();
        QVERIFY(!second.isEmpty() && first != second);
        QTRY_COMPARE(prompt->property("serverError").toString(), "Session was changed");
        QVERIFY(root->property("sessionPromptVisible").toBool());
        QVERIFY(!prompt->property("waiting").toBool());
        const auto writes = offlineRequests(dir);
        QCOMPARE(writes.size(), 2);
        const auto params = writes.last().value("params").toObject();
        QCOMPARE(params.value("op_id"), second);
        QCOMPARE(params.value("expected_version"), 4);
        QCOMPARE(params.value("values"), QJsonValue(QJsonObject{}));
        QCOMPARE(params.value("note"), QJsonValue(QJsonValue::Null));
    }

    void alreadyTickedCompletesPromptWithoutWriting() {
        QTemporaryDir dir;
        Backend backend(nullptr, fake(dir));
        backend.reload();
        waitForBackendIdle(backend);
        QSignalSpy finished(&backend, &Backend::sessionWriteFinished);
        backend.tickItem("<id:5>", {}, {}, "skip");
        backend.tickItem("<id:5>", {{"focus", 2}}, "unsaved note", "done");
        QCOMPARE(finished.size(), 2);
        QCOMPARE(finished[0].at(0).toBool(), true);
        QCOMPARE(finished[0].at(2).toString(), "skip");
        QCOMPARE(finished[1].at(0).toBool(), false);
        QVERIFY(finished[1].at(1).toString().contains("not saved"));
        QCOMPARE(finished[1].at(2).toString(), "done");
        QVERIFY(offlineRequests(dir).isEmpty());
    }

    void filteredChartsUseWeightedTotalsAndClearOnReadFailure() {
        QTemporaryDir dir;
        fake(dir);
        QJsonObject stats = fixture("stats");
        auto totals = stats.value("totals").toObject();
        totals["avg_focus"] = 4.6;
        stats["totals"] = totals;
        QJsonArray points{QJsonObject{{"day", "2031-05-13"}, {"value", 1}},
                          QJsonObject{{"day", "2031-05-14"}, {"value", 5}}};
        QJsonObject series{{"metric_id", "focus"}, {"name", "Focus"}, {"rollup", "avg"}, {"points", points}};
        auto custom = series;
        custom["metric_id"] = "clarity";
        stats["series"] = QJsonArray{series, custom};
        write(dir.filePath("report.json"), QJsonDocument(stats).toJson());
        const auto script = dir.filePath("report-cli");
        write(script, "#!/bin/sh\ncd '" + dir.path().toUtf8() + "'\n" + R"SH(
if [ "$1" = stats ]; then
    printf '%s\n' "$*" >> report-args
    case "$*" in
        *broken*) exit 1 ;;
        "stats table"*) echo '{"totals":{"values":{"clarity":2.4}}}' ;;
        *) cat report.json ;;
    esac
else
    exec ./salah "$@"
fi
)SH");
        QFile::setPermissions(script, QFile::ReadOwner | QFile::WriteOwner | QFile::ExeOwner);
        Backend backend(nullptr, script);
        Theme theme(dir.path());
        QQmlEngine engine;
        engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl("qrc:/qml/Main.qml"));
        QScopedPointer<QObject> root(component.create());
        QVERIFY2(root, qPrintable(component.errorString()));
        const QVariantMap filter{{"days", 7}, {"project_id", "none"}, {"epic_id", "none"},
            {"minutes_min", 10}, {"values", QVariantMap{{"focus", QVariantMap{{"min", 2}, {"max", 5}}}}}};
        backend.loadStatsQuery(filter);
        waitForBackendIdle(backend);
        auto *window = qobject_cast<QQuickWindow *>(root.data());
        auto focus = visualNamed(window, "metricChart_focus").value(0);
        auto clarity = visualNamed(window, "metricChart_clarity").value(0);
        QVERIFY(focus && clarity);
        QCOMPARE(focus->property("summary").toDouble(), 4.6);
        QCOMPARE(clarity->property("summary").toDouble(), 2.4);
        QFile args(dir.filePath("report-args"));
        QVERIFY(args.open(QIODevice::ReadOnly));
        const auto output = args.readAll();
        QVERIFY(output.contains("stats --days 7 --project none --epic none --min-minutes 10 --max focus=5 --min focus=2 --json"));
        QVERIFY(output.contains("stats table --days 7 --project none --epic none --min-minutes 10 --max focus=5 --min focus=2"));
        backend.loadStatsQuery({{"project_id", "broken"}});
        waitForBackendIdle(backend);
        QVERIFY(!backend.error().isEmpty());
        QVERIFY(backend.statsData().isEmpty());
    }

    void taskSearchIncludesUnassignedWorkBeyondFirstPage() {
        QTemporaryDir dir;
        const auto cli = fake(dir);
        const auto script = dir.filePath("items-cli");
        QJsonArray tasks;
        for (int n = 1; n <= 31; ++n)
            tasks.append(QJsonObject{{"id", QString::number(n)}, {"title", QString("Read chapter %1").arg(n)},
                                     {"project_id", n == 31 ? QJsonValue("book") : QJsonValue(QJsonValue::Null)}});
        write(dir.filePath("items.json"), QJsonDocument(tasks).toJson());
        write(script, "#!/bin/sh\nif [ \"$1 $2\" = 'item list' ]; then cat '" + dir.filePath("items.json").toUtf8()
              + "'; else exec '" + cli.toUtf8() + "' \"$@\"; fi\n");
        QFile::setPermissions(script, QFile::ReadOwner | QFile::WriteOwner | QFile::ExeOwner);
        Backend backend(nullptr, script);
        Theme theme(dir.path());
        QQmlEngine engine;
        engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl("qrc:/qml/Main.qml"));
        QScopedPointer<QObject> root(component.create());
        QVERIFY2(root, qPrintable(component.errorString()));
        root->setProperty("page", 2);
        backend.loadTrackingItems();
        waitForBackendIdle(backend);
        QObject *stats = findByName(root.data(), "statsScroll");
        QObject *bar = stats->property("filterBar").value<QObject *>();
        QVERIFY(bar);
        auto *none = findByName(bar, "statsProject_none");
        QVERIFY(none && QMetaObject::invokeMethod(none, "clicked"));
        QCOMPARE(bar->property("taskChoices").toList().size(), 30);
        auto *search = findByName(bar, "statsTaskSearch");
        QVERIFY(search);
        search->setProperty("text", "CHAPTER 30");
        QVERIFY(QMetaObject::invokeMethod(search, "textEdited"));
        QCOMPARE(bar->property("matchingTasks").toList().size(), 1);
        auto *last = findItemByName(bar, "statsTask_30");
        QVERIFY(last && QMetaObject::invokeMethod(last, "clicked"));
        QVariant query;
        QVERIFY(QMetaObject::invokeMethod(bar, "query", Q_RETURN_ARG(QVariant, query)));
        QCOMPARE(query.toMap().value("project_id").toString(), "none");
        QCOMPARE(query.toMap().value("item_id").toString(), "30");
    }

    void coreClientQueueCoalescingAndOrdering() {
        QTemporaryDir dir; CoreClient client(nullptr,fakeCore(dir),3000); QSignalSpy spy(&client,&CoreClient::requestFinished);
        int callbacks = 0;
        const auto callback = [&callbacks](const CoreClient::Result &result){ QVERIFY(result.ok); ++callbacks; };
        client.day(QDate(2099,1,1),callback); client.day(QDate(2099,1,1),callback);
        client.statsDays(7,callback); client.day(QDate(2099,1,1),callback);
        QTRY_COMPARE_WITH_TIMEOUT(spy.size(),2,5000);
        QCOMPARE(callbacks,4);
        QCOMPARE(coreLogLines(dir),QStringList({"<day><--date><2099-01-01><--json>","<stats><--days><7><--json>"}));
    }
    void coreClientTypedErrorsAndTimeoutKill() {
        QTemporaryDir dir; CoreClient client(nullptr,fakeCore(dir),100); QSignalSpy spy(&client,&CoreClient::requestFinished);
        QList<CoreClient::Result> results;
        client.parse("failure",[&](const CoreClient::Result &r){results<<r;});
        client.parse("malformed",[&](const CoreClient::Result &r){results<<r;});
        client.day(QDate(2099,1,1),[&](const CoreClient::Result &r){results<<r;});
        client.configShow([&](const CoreClient::Result &r){results<<r;});
        QTRY_COMPARE_WITH_TIMEOUT(spy.size(),4,5000);
        QCOMPARE(results.at(0).error.kind,CoreClient::ErrorKind::Exit); QCOMPARE(results.at(0).error.message,QString("requested failure"));
        QCOMPARE(results.at(1).error.kind,CoreClient::ErrorKind::Decode); QVERIFY(results.at(1).error.message.contains("invalid JSON"));
        QCOMPARE(results.at(2).error.kind,CoreClient::ErrorKind::Timeout); QVERIFY(results.at(2).error.message.contains("timed out"));
        QVERIFY(results.at(3).ok);
    }
    void realCliScreenshots() {
        const QString outputDir=qEnvironmentVariable("A9_SCREENSHOT_DIR");
        if(outputDir.isEmpty()) return;
        QDir().mkpath(outputDir);
        Backend backend(nullptr,qEnvironmentVariable("SALAH_BIN")); Theme theme;
        QSignalSpy spy(&backend,&Backend::requestFinished); backend.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(),4,5000);
        QQmlEngine engine; engine.rootContext()->setContextProperty("backend",&backend); engine.rootContext()->setContextProperty("theme",&theme);
        QQmlComponent component(&engine,QUrl::fromLocalFile(QStringLiteral(TEST_SOURCE_DIR "/../src/qml/Main.qml")));
        QScopedPointer<QObject> root(component.create()); QVERIFY2(root,qPrintable(component.errorString()));
        auto *window=qobject_cast<QQuickWindow *>(root.data()); QVERIFY(window); window->setWidth(1440);window->setHeight(900);window->show();QTest::qWait(250);
        auto shot=[&](const QString &name){QTest::qWait(200);QImage image=window->grabWindow();QVERIFY2(!image.isNull(),qPrintable(name));QVERIFY(image.save(outputDir+"/"+name));};
        shot("a9-today.png");
        root->setProperty("page",2);root->setProperty("statsMode","week");root->setProperty("statsAnchor",QDateTime(QDate(2026,11,22),QTime()));
        QVERIFY(QMetaObject::invokeMethod(root.data(),"loadStatistics"));QTRY_VERIFY_WITH_TIMEOUT(!backend.busy(),5000);shot("a9-week.png");
        root->setProperty("statsMode","month");root->setProperty("statsAnchor",QDateTime(QDate(2026,11,22),QTime()));
        QVERIFY(QMetaObject::invokeMethod(root.data(),"loadStatistics"));QTRY_VERIFY_WITH_TIMEOUT(!backend.busy(),5000);
        auto *scroll=root->findChild<QObject *>("statsScroll");QVERIFY(scroll);for(QObject *child:scroll->findChildren<QObject *>())if(child->property("contentY").isValid()){child->setProperty("contentY",240);break;}shot("a9-month.png");
        for(QObject *child:scroll->findChildren<QObject *>())if(child->property("contentY").isValid()){child->setProperty("contentY",0);break;}
        root->setProperty("statsTableVisible",true);shot("a9-table.png");
        root->setProperty("statsTableVisible",false);QTest::qWait(150);
        // Month chart plot spans x=298..1396. Day 17 is column index 16.
        QTest::mouseMove(window,QPoint(902,450));auto *tooltip=root->findChild<QObject *>("statsTooltip");QVERIFY(tooltip);QTRY_VERIFY_WITH_TIMEOUT(tooltip->property("visible").toBool(),1000);QCOMPARE(tooltip->property("text").toString(),QString("Tue 17 Nov · 2 / 2 done · 105 min · 9 counts · 3 prayers"));shot("a9-tooltip.png");
        window->hide();
    }
    void isolatedRealCliSmoke() {
        if (qEnvironmentVariableIsEmpty("SALAH_APP_SMOKE")) return;
        Backend backend(nullptr,qEnvironmentVariable("SALAH_BIN")); Theme theme;
        QSignalSpy spy(&backend,&Backend::requestFinished); backend.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(),4,5000);
        QVERIFY2(backend.error().isEmpty(),qPrintable(backend.error()));
        QQmlEngine engine; engine.rootContext()->setContextProperty("backend",&backend); engine.rootContext()->setContextProperty("theme",&theme);
        QQmlComponent component(&engine,QUrl::fromLocalFile(QStringLiteral(TEST_SOURCE_DIR "/../src/qml/Main.qml")));
        QScopedPointer<QObject> root(component.create()); QVERIFY2(root,qPrintable(component.errorString())); root->setProperty("visible",false);
        root->setProperty("page",2); root->setProperty("statsMode","week");
        QVERIFY(QMetaObject::invokeMethod(root.data(),"loadStatistics")); QTRY_VERIFY_WITH_TIMEOUT(!backend.busy(),5000);
        QVERIFY2(backend.error().isEmpty(),qPrintable(backend.error())); QVERIFY(backend.statsItems()->rowCount() > 0);
    }
    void routinesCatalogActionsAndHistory() {
        QTemporaryDir dir;
        fake(dir);
        const QString program = dir.filePath("routine-cli");
        write(program, R"SH(#!/bin/sh
cd "$(dirname "$0")"
case "$1 $2" in
  "item list") cat catalog.json ;;
  "sessions list")
    case "$*" in
      *"--before older"*) cat older.json ;;
      *) cat recent.json ;;
    esac ;;
  *) exec ./salah "$@" ;;
esac
)SH");
        QFile::setPermissions(program, QFile::ReadOwner | QFile::WriteOwner | QFile::ExeOwner);
        auto catalog = rpcFixture("items_list").value("result").toArray();
        QJsonObject weekly = catalog.first().toObject();
        weekly["id"] = "weekly";
        weekly["title"] = "Review grammar notes";
        weekly["block"] = "isha";
        weekly["repeat"] = "weekly";
        weekly["weekdays"] = QJsonArray{"mon", "fri"};
        weekly["schedule_text"] = "Mon, Fri";
        weekly["next_occurrence"] = "2031-05-16";
        weekly["project_id"] = "study";
        weekly["epic_id"] = "e-grammar";
        weekly["metric_ids"] = QJsonArray{"focus"};
        weekly["tag_ids"] = QJsonArray{"learning"};
        weekly["version"] = 7;
        catalog.append(weekly);
        write(dir.filePath("catalog.json"), QJsonDocument(catalog).toJson());
        auto session = [](const QString &day, const QJsonValue &minutes, const QString &item) {
            return QJsonObject{{"day", day}, {"minutes", minutes}, {"item_id", item}};
        };
        write(dir.filePath("recent.json"), QJsonDocument(QJsonObject{{"next", "older"}, {"rows", QJsonArray{
            session("2031-05-14", 20, "weekly"), session("2031-05-12", QJsonValue::Null, "weekly"),
            session("2031-05-12", QJsonValue::Null, "weekly"), session("2031-05-12", 30, "<id:4>")}}}).toJson());
        write(dir.filePath("older.json"), QJsonDocument(QJsonObject{{"next", QJsonValue::Null}, {"rows", QJsonArray{
            session("2031-05-09", QJsonValue::Null, "weekly"), session("2031-05-09", 15, "weekly")}}}).toJson());
        TrackingUiPreviewBackend backend(program, daemon_path(dir), catalog, {});
        backend.reload();
        waitForBackendIdle(backend);
        Theme theme(dir.path());
        QQmlEngine engine;
        engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl("qrc:/qml/Main.qml"));
        QScopedPointer<QObject> root(component.create());
        QVERIFY2(root, qPrintable(component.errorString()));
        auto *window = qobject_cast<QQuickWindow *>(root.data());
        window->resize(1080, 740);
        window->setProperty("page", 1);
        QTRY_VERIFY(hasVisualText(window, "2 days done  ·  35 min"));
        QVERIFY(hasVisualText(window, "Done days · last 14 days"));
        QCOMPARE(backend.routines()->rowCount(), 2);
        shotWindow(window, "routines.png");
        auto *row = qobject_cast<QQuickItem *>(firstShown(window, "routineRow_weekly"));
        QVERIFY(row);
        const auto strip = row->property("doneStrip").toList();
        QCOMPARE(strip.size(), 14);
        QVERIFY(strip[8].toBool() && strip[11].toBool() && !strip.last().toBool());
        QTest::mouseClick(window, Qt::LeftButton, {}, row->mapToScene(QPointF(40, 28)).toPoint());
        auto *form = findByName(window, "entryPopup");
        QTRY_VERIFY(form && form->property("opened").toBool());
        QCOMPARE(form->property("repeat").toString(), QString("weekly"));
        QCOMPARE(form->property("days").toStringList(), QStringList({"mon", "fri"}));
        QCOMPARE(form->property("epicId").toString(), QString("e-grammar"));
        QCOMPARE(form->property("tagIds").toStringList(), QStringList{"learning"});
        QCOMPARE(form->property("metricIds").toStringList(), QStringList{"focus"});
        shotWindow(window, "routine-editor.png");
        QVERIFY(QMetaObject::invokeMethod(firstShown(window, "routineResources"), "clicked"));
        auto *drawer = findByName(window, "resourceDrawer");
        QTRY_VERIFY(drawer && drawer->property("opened").toBool());
        QCOMPARE(drawer->property("targetId").toString(), QString("weekly"));
        QVERIFY(QMetaObject::invokeMethod(drawer, "close"));
        QTRY_VERIFY(!drawer->property("opened").toBool());
        QVERIFY(QMetaObject::invokeMethod(form, "submitTitle", Q_ARG(QVariant, false)));
        QTRY_VERIFY(!offlineRequests(dir).isEmpty());
        const auto edit = offlineRequests(dir).first();
        QCOMPARE(edit.value("method").toString(), QString("item.edit"));
        QCOMPARE(edit.value("params").toObject().value("expected_version").toInt(), 7);
        QCOMPARE(edit.value("params").toObject().value("id").toString(), QString("weekly"));
        QTRY_VERIFY(!form->property("opened").toBool());
        QVERIFY(QMetaObject::invokeMethod(findItemByName(row, "routineEdit"), "clicked"));
        QTRY_VERIFY(form->property("opened").toBool());
        window->setProperty("entryVisible", false);
        QTRY_VERIFY(!form->property("opened").toBool());
        QVERIFY(QMetaObject::invokeMethod(findItemByName(row, "routineRemove"), "clicked"));
        QVERIFY(hasVisualText(window, "Confirm removal"));
        QCOMPARE(offlineRequests(dir).size(), 1);
        shotWindow(window, "routine-remove.png");
        write(dir.filePath("catalog.json"), "[]");
        QVERIFY(QMetaObject::invokeMethod(findItemByName(row, "routineRemove"), "clicked"));
        QTRY_COMPARE(offlineRequests(dir).size(), 2);
        const auto remove = offlineRequests(dir).last();
        QCOMPARE(remove.value("method").toString(), QString("item.remove"));
        QCOMPARE(remove.value("params").toObject().value("expected_version").toInt(), 7);
        QTRY_VERIFY(hasVisualText(window, "No routines yet"));
        shotWindow(window, "routines-empty.png");
        QVERIFY(QMetaObject::invokeMethod(firstShown(window, "newRoutineEmpty"), "clicked"));
        QTRY_VERIFY(form->property("opened").toBool());
        QCOMPARE(form->property("repeat").toString(), QString("daily"));
        auto *title = findByName(window, "entryTitleField");
        QVERIFY(title);
        title->setProperty("text", "Read after Fajr");
        QVERIFY(QMetaObject::invokeMethod(form, "submitTitle", Q_ARG(QVariant, false)));
        QTRY_COMPARE(offlineRequests(dir).size(), 3);
        const auto add = offlineRequests(dir).last();
        QCOMPARE(add.value("method").toString(), QString("item.add"));
        QCOMPARE(add.value("params").toObject().value("repeat").toString(), QString("daily"));
        QCOMPARE(add.value("params").toObject().value("title").toString(), QString("Read after Fajr"));
    }

    void resourcesAtTaskCreation() {
        QTemporaryDir dir;
        fake(dir);
        const QString program = dir.filePath("attachment-cli");
        write(program, R"SH(#!/bin/sh
cd "$(dirname "$0")"
if [ "$1" != rpc ]; then exec ./salah "$@"; fi
read -r hello && read -r request || exit 2
printf '%s\n' "$request" >> frames
case "$request" in
 *'resource.add'*'https://example.org/read'*)
    if [ ! -f failed ]; then
        touch failed
        printf '%s\n' "$request" | jq -c '{jsonrpc,id,error:{code:-32001,message:"Temporary write failure",data:{kind:"storage"}}}'
        exit 0
    fi ;;
esac
printf '%s\n' "$request" | jq -c '{jsonrpc,id,result:{op_id:.params.op_id,revision:1,changed:{items:(if .method=="item.add" then [{id:"new-owner"}] else [] end),sessions:[],projects:[],tags:[],metrics:[],marks:[],deleted:[]}}}'
)SH");
        QFile::setPermissions(program, QFile::ReadOwner|QFile::WriteOwner|QFile::ExeOwner);
        TrackingUiPreviewBackend backend(program, daemon_path(dir), {}, {});
        Theme theme(dir.path()); QQmlEngine engine;
        engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl("qrc:/qml/Main.qml"));
        QScopedPointer<QObject> root(component.create()); QVERIFY2(root, qPrintable(component.errorString()));
        auto *window = qobject_cast<QQuickWindow *>(root.data());
        window->resize(1440, 1020);
        auto click = [&](const QString &name) {
            QObject *button = firstShown(window, name);
            return button && QMetaObject::invokeMethod(button, "clicked");
        };
        for (bool backlog : {false, true}) {
            window->setProperty("page", backlog ? 5 : 0);
            QVERIFY(QMetaObject::invokeMethod(root.data(), "openEntry", Q_ARG(QVariant, (QVariantMap{{"backlog", backlog}}))));
            auto *form = findByName(window, "entryPopup");
            QTRY_VERIFY(form->property("opened").toBool());
            firstShown(window, "entryTitleField")->setProperty("text", "Read chapter 3");
            QVERIFY(click("composeAttachment"));
            firstShown(window, "resourceBody")->setProperty("text", "Read before class");
            QVERIFY(click("resourceSave"));
            QVERIFY(click("composeAttachment"));
            QVERIFY(click("resourceKind_link"));
            auto *url = firstShown(window, "resourceUrl");
            url->setProperty("text", "not an address");
            QVERIFY(!firstShown(window, "resourceSave")->property("enabled").toBool());
            QVERIFY(hasVisualText(window, "Enter a web address, e.g. example.com"));
            if (backlog) shotWindow(window, "new-task-link-hint.png");
            url->setProperty("text", "example.org/read");
            QVERIFY(firstShown(window, "resourceSave")->property("enabled").toBool());
            QVERIFY(click("resourceSave"));
            QVERIFY(click("composeAttachment"));
            QVERIFY(click("resourceKind_reminder"));
            firstShown(window, "resourceTitle")->setProperty("text", "Bring the book");
            QVERIFY(click("resourceSave"));
            QVERIFY(hasVisualText(window, "https://example.org/read"));
            shotWindow(window, backlog ? "new-backlog-resources.png" : "new-today-resources.png");
            const int start = offlineRequests(dir).size();
            QVERIFY(QMetaObject::invokeMethod(form, "submitTitle", Q_ARG(QVariant, false)));
            if (!backlog) {
                QTRY_VERIFY(form->property("saveError").toString().contains("Task saved"));
                shotWindow(window, "new-task-resource-retry.png");
                QVERIFY(QMetaObject::invokeMethod(form, "submitTitle", Q_ARG(QVariant, false)));
            }
            QTRY_VERIFY(!form->property("opened").toBool());
            const auto writes = offlineRequests(dir).mid(start);
            QCOMPARE(writes.size(), backlog ? 4 : 5);
            QCOMPARE(writes.first().value("method").toString(), QString("item.add"));
            QCOMPARE(writes.first().value("params").toObject().value("backlog").toBool(), backlog);
            for (int i = 1; i < writes.size(); ++i) {
                QCOMPARE(writes[i].value("method").toString(), QString("resource.add"));
                QCOMPARE(writes[i].value("params").toObject().value("item_id").toString(), QString("new-owner"));
            }
            if (!backlog) QCOMPARE(writes[2].value("params"), writes[3].value("params"));
            QCOMPARE(writes.last().value("params").toObject().value("kind").toString(), QString("reminder"));
        }
    }

    void statisticsIncludesSessionsBeforeEpicAssignment() {
        QTemporaryDir dir;
        fake(dir);
        const QJsonObject report = fixture("stats_epic_reassigned");
        auto epic = report.value("epics").toArray().first().toObject();
        const auto id = epic.value("id").toString();
        epic["project_id"] = "study";
        write(dir.filePath("reassigned.json"), QJsonDocument(report).toJson());
        const QString cli = dir.filePath("epic-cli");
        write(cli, R"SH(#!/bin/sh
cd "$(dirname "$0")"
case "$1 $*" in
  "stats stats --days 28 --epic <id:1> --json") cat reassigned.json ;;
  *) exec ./salah "$@" ;;
esac
)SH");
        QFile::setPermissions(cli, QFile::ReadOwner|QFile::WriteOwner|QFile::ExeOwner);
        TrackingUiPreviewBackend backend(cli, daemon_path(dir), {}, {});
        backend.epicCatalog = {epic.toVariantMap()};
        Theme theme(dir.path()); QQmlEngine engine;
        engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl("qrc:/qml/Main.qml"));
        QScopedPointer<QObject> root(component.create()); QVERIFY2(root, qPrintable(component.errorString()));
        auto *window = qobject_cast<QQuickWindow *>(root.data());
        window->resize(1280, 900); window->setProperty("page", 2);
        auto *stats = findByName(window, "statsScroll");
        QVERIFY(QMetaObject::invokeMethod(stats, "setTab", Q_ARG(QVariant, "charts")));
        auto *chip = firstShown(window, "statsEpic_" + id); QVERIFY(chip);
        QVERIFY(QMetaObject::invokeMethod(chip, "clicked"));
        QTRY_COMPARE(backend.statsData().value("filter").toMap().value("epic_id").toString(), id);
        QCOMPARE(backend.statsData().value("totals").toMap().value("minutes").toInt(), 30);
        QVERIFY(hasVisualText(window, "10 sessions"));
        shotWindow(window, "stats-reassigned-epic.png");
        auto *bar = stats->property("filterBar").value<QObject *>();
        QVERIFY(bar);
        QVERIFY(QMetaObject::invokeMethod(bar, "setFilter", Q_ARG(QVariant, "tag"), Q_ARG(QVariant, "learning")));
        QCOMPARE(bar->property("tagId").toString(), QString("learning"));
        waitForBackendIdle(backend);
        backend.tagCatalog.clear();
        QSignalSpy requests(&backend, &Backend::requestFinished);
        emit backend.tagsLoaded();
        QCOMPARE(bar->property("tagId").toString(), QString());
        QCOMPARE(bar->property("epicId").toString(), id);
        QTRY_VERIFY(!requests.isEmpty());
        QCOMPARE(backend.statsData().value("filter").toMap().value("epic_id").toString(), id);
        shotWindow(window, "stats-removed-tag.png");
    }

    void asyncQueueAndModels() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir)); QSignalSpy spy(&b,&Backend::requestFinished);
        b.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(),4,3000);
        QCOMPARE(b.todayDay(),QString("2031-05-14")); QCOMPARE(b.prayers()->rowCount(),5);
        QVERIFY(b.routines()->rowCount() > 0);
        QCOMPARE(b.unplannedMinutes(), 20);
        // day.get carries the planned and plan-line sums; the app derives the ratio.
        QVERIFY(qAbs(b.fulfilmentRatio().toDouble() - 25.0 / 45.0) < 0.001);
        bool sawCapacity = false;
        for (int row = 0; row < b.dayTimeline()->rowCount(); ++row) {
            const QModelIndex index = b.dayTimeline()->index(row, 0);
            if (b.dayTimeline()->data(index, DayTimelineModel::RowKindRole).toString() == "header"
                && b.dayTimeline()->data(index, DayTimelineModel::CapacityMinutesRole).toInt() > 0)
                sawCapacity = true;
        }
        QVERIFY(sawCapacity);
        QCOMPARE(b.config()->location(),QString("Tashkent"));
        QCOMPARE(logLines(dir).mid(0,3),QStringList({"day --json","item list --json","config show"}));
    }
    void timeoutReportsAndQueueContinues() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir),100); QSignalSpy spy(&b,&Backend::requestFinished);
        b.parseQuick("slow"); b.reload(); QTRY_VERIFY_WITH_TIMEOUT(spy.size()>=4,3000); QVERIFY(b.error().contains("timed out"));
    }
    void exactWriteArguments() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir)); QSignalSpy spy(&b,&Backend::requestFinished);
        b.reload(); QTRY_COMPARE(spy.size(),4); b.addQuick("Read Quran after Isha","asr");
        b.moveItem("a1", "isha"); b.togglePrayer("fajr", false);
        QTRY_COMPARE_WITH_TIMEOUT(offlineRequests(dir).size(), 3, 3000);
        const auto requests = offlineRequests(dir);
        const auto add = requests[0].value("params").toObject();
        QCOMPARE(add.value("quick_text"), "Read Quran after Isha");
        QCOMPARE(add.value("block"), "asr");
        QCOMPARE(requests[1].value("params").toObject().value("index"), 0);
        QCOMPARE(requests[2].value("params").toObject().value("date"), "2031-05-14");
    }
    void socketMoveSendsNeighbourAnchors() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        auto reads = readHandler(fixture("day"));
        daemon.setHandler([reads](const QJsonObject &request, QLocalSocket *socket, FakeDaemon *server) {
            if (request.value("method").toString() == "item.move") {
                server->respond(socket, request, QJsonObject{{"revision", 101},
                    {"op_id", request.value("params").toObject().value("op_id")},
                    {"changed", QJsonObject{{"items", QJsonArray{}}, {"sessions", QJsonArray{}},
                                            {"projects", QJsonArray{}}, {"tags", QJsonArray{}},
                                            {"metrics", QJsonArray{}}, {"marks", QJsonArray{}},
                                            {"deleted", QJsonArray{}}}}});
                return;
            }
            reads(request, socket, server);
        });
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy spy(&backend, &Backend::requestFinished);
        backend.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 4, 3000);
        backend.moveItem("<id:4>", "fajr", "<id:5>", {});
        auto moveSent = [&] {
            for (const QJsonObject &request : std::as_const(daemon.requests))
                if (request.value("method").toString() == "item.move") return true;
            return false;
        };
        QTRY_VERIFY_WITH_TIMEOUT(moveSent(), 3000);
        QJsonObject moveParams;
        for (const QJsonObject &request : std::as_const(daemon.requests))
            if (request.value("method").toString() == "item.move") moveParams = request.value("params").toObject();
        QCOMPARE(moveParams.value("block").toString(), QString("fajr"));
        QCOMPARE(moveParams.value("after_id").toString(), QString("<id:5>"));
        QVERIFY(!moveParams.contains("before_id"));
        QVERIFY(!moveParams.contains("index"));
    }
    void unchangedJsonDoesNotResetDayModel() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir)); QSignalSpy spy(&b,&Backend::requestFinished);
        b.reload(); QTRY_COMPARE(spy.size(),4);
        auto *model = b.dayTimeline();
        QSignalSpy changed(model, &QAbstractItemModel::dataChanged);
        QSignalSpy inserted(model, &QAbstractItemModel::rowsInserted);
        QSignalSpy removed(model, &QAbstractItemModel::rowsRemoved);
        QSignalSpy moved(model, &QAbstractItemModel::rowsMoved);
        QSignalSpy reset(model, &QAbstractItemModel::modelReset);
        b.reload(); QTRY_COMPARE(spy.size(),8); QTest::qWait(50);
        QCOMPARE(changed.size(), 0); QCOMPARE(inserted.size(), 0);
        QCOMPARE(removed.size(), 0); QCOMPARE(moved.size(), 0); QCOMPARE(reset.size(), 0);
    }
    void pendingTickWaitsForCanonicalResponse() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir)); QSignalSpy spy(&b,&Backend::requestFinished);
        // <id:5> has only a tick today: done again removes that tick, and the
        // row stays pending (and done) until the canonical refetch.
        b.reload(); QTRY_COMPARE(spy.size(),4); QVERIFY(b.toggleDone("<id:5>","done"));
        const int row = timelineItem(b.dayTimeline(), "<id:5>");
        QVERIFY(row >= 0);
        QCOMPARE(b.dayTimeline()->data(b.dayTimeline()->index(row, 0), DayTimelineModel::StateRole).toString(),QString("done"));
        QVERIFY(b.dayTimeline()->data(b.dayTimeline()->index(row, 0), DayTimelineModel::PendingRole).toBool());
        QVERIFY(b.busy());
    }
    void sessionStartMarksOnlyTheTouchedRowPending() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir)); QSignalSpy spy(&b,&Backend::requestFinished);
        b.reload(); QTRY_COMPARE(spy.size(),4); b.toggleTimer("<id:4>",false);
        const int untouched = timelineItem(b.dayTimeline(), "<id:5>");
        const int touched = timelineItem(b.dayTimeline(), "<id:4>");
        QVERIFY(untouched >= 0); QVERIFY(touched >= 0);
        QVERIFY(!b.dayTimeline()->data(b.dayTimeline()->index(untouched, 0), DayTimelineModel::PendingRole).toBool());
        QVERIFY(b.dayTimeline()->data(b.dayTimeline()->index(touched, 0), DayTimelineModel::PendingRole).toBool());
        QTRY_COMPARE_WITH_TIMEOUT(offlineRequests(dir).size(), 1, 3000);
        QCOMPARE(offlineRequests(dir).first().value("params").toObject().value("item_id"), "<id:4>");
    }
    void escapeFunctionClosesTopOverlay() {
        QTemporaryDir dir; Backend backend(nullptr,fake(dir)); Theme theme(dir.path());
        QQmlEngine engine; engine.rootContext()->setContextProperty("backend",&backend); engine.rootContext()->setContextProperty("theme",&theme);
        QQmlComponent component(&engine,QUrl::fromLocalFile(QStringLiteral(TEST_SOURCE_DIR "/../src/qml/Main.qml")));
        QScopedPointer<QObject> root(component.create());
        QVERIFY2(root, qPrintable(component.errorString())); root->setProperty("visible",false);
        root->setProperty("quickVisible",true); root->setProperty("helpVisible",true);
        QVariant result; QVERIFY(QMetaObject::invokeMethod(root.data(),"closeTopOverlay",Q_RETURN_ARG(QVariant,result))); QVERIFY(result.toBool());
        QCOMPARE(root->property("helpVisible").toBool(),false); QCOMPARE(root->property("quickVisible").toBool(),true);
        QVERIFY(QMetaObject::invokeMethod(root.data(),"closeTopOverlay",Q_RETURN_ARG(QVariant,result)));
        QCOMPARE(root->property("quickVisible").toBool(),false);
        QVERIFY(QMetaObject::invokeMethod(root.data(),"closeTopOverlay",Q_RETURN_ARG(QVariant,result))); QVERIFY(!result.toBool());
    }
    void weeklyGuardAndDefaultDay() {
        QTemporaryDir dir; Backend backend(nullptr,fake(dir)); Theme theme(dir.path());
        backend.reload(); QSignalSpy spy(&backend,&Backend::requestFinished); QTRY_VERIFY_WITH_TIMEOUT(spy.size()>=3,3000);
        QQmlEngine engine; engine.rootContext()->setContextProperty("backend",&backend); engine.rootContext()->setContextProperty("theme",&theme);
        QQmlComponent component(&engine,QUrl::fromLocalFile(QStringLiteral(TEST_SOURCE_DIR "/../src/qml/Main.qml")));
        QScopedPointer<QObject> root(component.create()); QVERIFY2(root,qPrintable(component.errorString())); root->setProperty("visible",false);
        QObject *entry=root->findChild<QObject *>("entryPopup"); QVERIFY(entry);
        entry->setProperty("days",QVariantList{});
        QVERIFY(QMetaObject::invokeMethod(entry,"selectRepeat",Q_ARG(QVariant,QVariant("weekly"))));
        QCOMPARE(entry->property("days").toList().size(), 1); QVERIFY(entry->property("weeklyValid").toBool());
        entry->setProperty("days",QVariantList{}); QVERIFY(!entry->property("weeklyValid").toBool());
    }
    void hijriHeaderFormatting() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir)); const QString h=b.hijriHeader("2026-11-22");
        QVERIFY(h.startsWith("Sunday ")); QVERIFY(h.contains("Jumada")); QCOMPARE(b.gregorianHeader("2026-11-22"),QString("November 22, 2026"));
    }
    void statsRangeQueryAndModels() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir)); QSignalSpy spy(&b,&Backend::requestFinished);
        b.loadStatsRange("2026-11-01","2026-11-30"); QTRY_COMPARE(spy.size(),1);
        QCOMPARE(logLines(dir).last(),QString("stats --from 2026-11-01 --to 2026-11-30 --json"));
        QCOMPARE(b.statsDaily()->data(b.statsDaily()->index(0, 0), StatsDailyModel::MinutesRole).toInt(),0);
        QCOMPARE(b.statsTotals()->activeDays(),1);
    }
    void statsRangeMathAndTooltip() {
        QTemporaryDir dir; Backend backend(nullptr,fake(dir)); Theme theme(dir.path());
        backend.loadStatsRange("2026-11-01","2026-11-30"); QSignalSpy statsSpy(&backend,&Backend::requestFinished); QTRY_VERIFY_WITH_TIMEOUT(statsSpy.size()>=1,3000);
        QQmlEngine engine; engine.rootContext()->setContextProperty("backend",&backend); engine.rootContext()->setContextProperty("theme",&theme);
        QQmlComponent component(&engine,QUrl::fromLocalFile(QStringLiteral(TEST_SOURCE_DIR "/../src/qml/Main.qml")));
        QScopedPointer<QObject> root(component.create()); QVERIFY2(root,qPrintable(component.errorString())); root->setProperty("visible",false);
        QVariant bounds; QVERIFY(QMetaObject::invokeMethod(root.data(),"statsBounds",Q_RETURN_ARG(QVariant,bounds),Q_ARG(QVariant,"week"),Q_ARG(QVariant,QDateTime(QDate(2026,11,22),QTime())),Q_ARG(QVariant,QDateTime()),Q_ARG(QVariant,QDateTime())));
        QCOMPARE(bounds.toMap().value("from").toString(),QString("2026-11-16")); QCOMPARE(bounds.toMap().value("to").toString(),QString("2026-11-22"));
        QVERIFY(QMetaObject::invokeMethod(root.data(),"statsBounds",Q_RETURN_ARG(QVariant,bounds),Q_ARG(QVariant,"month"),Q_ARG(QVariant,QDateTime(QDate(2026,2,15),QTime())),Q_ARG(QVariant,QDateTime()),Q_ARG(QVariant,QDateTime())));
        QCOMPARE(bounds.toMap().value("from").toString(),QString("2026-02-01")); QCOMPARE(bounds.toMap().value("to").toString(),QString("2026-02-28"));
        QVERIFY(QMetaObject::invokeMethod(root.data(),"statsBounds",Q_RETURN_ARG(QVariant,bounds),Q_ARG(QVariant,"custom"),Q_ARG(QVariant,QDateTime()),Q_ARG(QVariant,QDateTime(QDate(2026,12,2),QTime())),Q_ARG(QVariant,QDateTime(QDate(2026,11,29),QTime()))));
        QCOMPARE(bounds.toMap().value("from").toString(),QString("2026-11-29")); QCOMPARE(bounds.toMap().value("to").toString(),QString("2026-12-02"));
        QObject *bar = findByName(root.data(), "statsScroll")->property("filterBar").value<QObject *>();
        QVERIFY(bar);
        QVERIFY(QMetaObject::invokeMethod(bar, "setCustom", Q_ARG(QVariant, "2026-11-29"), Q_ARG(QVariant, "2026-12-02")));
        QVERIFY(QMetaObject::invokeMethod(root.data(), "stepStats", Q_ARG(QVariant, 1)));
        QCOMPARE(bar->property("customFrom").toString(), "2026-12-03");
        QCOMPARE(bar->property("customTo").toString(), "2026-12-06");
        QVERIFY(QMetaObject::invokeMethod(root.data(), "stepStats", Q_ARG(QVariant, -1)));
        QCOMPARE(bar->property("customTo").toString(), "2026-12-02");
        QSignalSpy edited(bar, SIGNAL(edited()));
        for (const auto &end : {"2026-02-30", "2026-04-30", "2028-11-29"}) {
            QVERIFY(QMetaObject::invokeMethod(bar, "setCustom", Q_ARG(QVariant, "2026-11-29"), Q_ARG(QVariant, end)));
            QVERIFY(!bar->property("rangeError").toString().isEmpty());
        }
        QCOMPARE(edited.size(), 0);
        QCOMPARE(bar->property("customFrom").toString(), "2026-11-29");
        QVariant stepped;
        QVERIFY(QMetaObject::invokeMethod(root.data(),"monthStep",Q_RETURN_ARG(QVariant,stepped),Q_ARG(QVariant,QDateTime(QDate(2024,1,31),QTime())),Q_ARG(QVariant,1)));
        QCOMPARE(stepped.toDateTime().date(),QDate(2024,2,29));
        QVERIFY(QMetaObject::invokeMethod(root.data(),"monthStep",Q_RETURN_ARG(QVariant,stepped),Q_ARG(QVariant,QDateTime(QDate(2026,3,31),QTime())),Q_ARG(QVariant,-1)));
        QCOMPARE(stepped.toDateTime().date(),QDate(2026,2,28));
        QVERIFY(QMetaObject::invokeMethod(root.data(),"monthStep",Q_RETURN_ARG(QVariant,stepped),Q_ARG(QVariant,QDateTime(QDate(2026,12,15),QTime())),Q_ARG(QVariant,1)));
        QCOMPARE(stepped.toDateTime().date(),QDate(2027,1,15));
        QVERIFY(QMetaObject::invokeMethod(root.data(),"monthStep",Q_RETURN_ARG(QVariant,stepped),Q_ARG(QVariant,QDateTime(QDate(2026,1,15),QTime())),Q_ARG(QVariant,-1)));
        QCOMPARE(stepped.toDateTime().date(),QDate(2025,12,15));
    }
    void coreProjectionTextDrivesApp() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        QJsonObject day = fixture("day");
        QJsonObject stats = fixture("stats");
        const QJsonObject preview = fixture("item_preview");
        daemon.setHandler([day, stats](const QJsonObject &request, QLocalSocket *socket, FakeDaemon *server) {
            const QString method = request.value("method").toString();
            if (method == "day.get") server->respond(socket, request, day);
            else if (method == "stats.get") server->respond(socket, request, stats);
            else if (method == "config.get") server->respond(socket, request, QJsonObject{{"sleep_time", "23:00"}, {"location", "Tashkent"}, {"latitude", 41.2}, {"longitude", 69.2}, {"timezone_offset", 5}, {"method", "uzbekistan"}, {"asr", "hanafi"}, {"auto_location", true}, {"last_location_provider", QJsonValue::Null}, {"last_location_sync", QJsonValue::Null}});
            else if (method == "item.preview") server->respond(socket, request, fixture("item_preview"));
            else server->respond(socket, request, QJsonObject{});
        });
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy spy(&backend, &Backend::requestFinished);
        backend.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 4, 3000);
        const int row = timelineItem(backend.dayTimeline(), "<id:4>");
        QVERIFY(row >= 0);
        auto role = [&](int r) { return backend.dayTimeline()->data(backend.dayTimeline()->index(row, 0), r); };
        QCOMPARE(role(DayTimelineModel::PlannedMinutesRole).toInt(), 30);
        QCOMPARE(role(DayTimelineModel::ActualMinutesRole).toInt(), 25);
        QCOMPARE(role(DayTimelineModel::SessionsRole).toInt(), 1);
        QVERIFY(!role(DayTimelineModel::ProjectColorRole).toString().isEmpty());
        backend.loadStatsRange("2031-05-08", "2031-05-14");
        QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 5, 3000);
        const QVariantList series = backend.statsData().value("series").toList();
        QVERIFY(!series.isEmpty());
        QCOMPARE(series.first().toMap().value("metric_id").toString(), QString("minutes"));

        auto decodedDay = Dto::decode<Dto::DayPlan>(day);
        QVERIFY(decodedDay);
        QVERIFY(!decodedDay.value->blocks.empty());
        auto decodedStats = Dto::decode<Dto::Stats>(stats);
        QVERIFY(decodedStats);
        QVERIFY(!decodedStats.value->json.value("series").toArray().isEmpty());
        auto decodedPreview = Dto::decode<Dto::Preview>(preview);
        QVERIFY(decodedPreview);
        QCOMPARE(decodedPreview.value->occurrences_this_month, 8);

        Theme theme(dir.path());
        QQmlEngine engine; engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl::fromLocalFile(QStringLiteral(TEST_SOURCE_DIR "/../src/qml/Main.qml")));
        QScopedPointer<QObject> root(component.create()); QVERIFY2(root, qPrintable(component.errorString()));
        root->setProperty("visible", true);
        QVERIFY(QMetaObject::invokeMethod(root.data(), "syncTimeline"));
        QTest::qWait(150);
        QStringList labels;
        for (QObject *object : root->findChildren<QObject *>())
            if (object->property("text").isValid()) labels << object->property("text").toString();
        QTRY_VERIFY_WITH_TIMEOUT(!backend.busy(), 3000);
        QTest::qWait(100);
        const int beforeReload = spy.size();
        backend.reload();
        QTRY_VERIFY_WITH_TIMEOUT(spy.size() >= beforeReload + 4, 3000);
        QTest::qWait(200);

        QQmlComponent cardComponent(&engine, QUrl::fromLocalFile(QStringLiteral(TEST_SOURCE_DIR "/../src/qml/today/ItemCard.qml")));
        QScopedPointer<QObject> card(cardComponent.createWithInitialProperties(
            {{"win", QVariant::fromValue(root.data())},
             {"itemId", role(DayTimelineModel::ItemIdRole)},
             {"title", role(DayTimelineModel::TitleRole)},
             {"state", role(DayTimelineModel::StateRole)},
             {"detailText", role(DayTimelineModel::DetailTextRole)},
             {"pending", role(DayTimelineModel::PendingRole)},
             {"projectColor", role(DayTimelineModel::ProjectColorRole)},
             {"plannedMinutes", role(DayTimelineModel::PlannedMinutesRole)},
             {"actualMinutes", role(DayTimelineModel::ActualMinutesRole)},
             {"sessions", role(DayTimelineModel::SessionsRole)}}));
        QVERIFY2(card, qPrintable(cardComponent.errorString()));
        QTest::qWait(50);
        QVERIFY(hasLabelText(card.data(), "1 session"));
        QVERIFY(hasLabelText(card.data(), "25 / 30 min · Daily"));

        backend.previewItem(QVariantMap{{"block", "asr"}, {"repeat", "weekly"},
            {"weekdays", QVariantList{"mon", "wed", "fri"}}, {"planned_minutes", 30}});
        QTRY_COMPARE_WITH_TIMEOUT(backend.preview()->scheduleText(), QString("Mon, Wed, Fri"), 3000);
        QObject *entry = root->findChild<QObject *>("entryPopup"); QVERIFY(entry);
        root->setProperty("entryVisible", true); entry->setProperty("visible", true);
        QTRY_VERIFY_WITH_TIMEOUT(hasLabelText(root.data(), "Mon, Wed, Fri"), 3000);
        QVERIFY(hasLabelText(root.data(), "May 14, 2031"));
        root->setProperty("visible", false);
    }

    void timelineDragIntoEmptyBlockShowsGapAndSendsAnchors() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        QJsonObject day = dragDay();
        daemon.setHandler([day](const QJsonObject &request, QLocalSocket *socket, FakeDaemon *server) mutable {
            if (request.value("method").toString() == "item.move") {
                const QJsonObject params = request.value("params").toObject();
                day = movedDay(day, params);
                server->respond(socket, request, QJsonObject{
                    {"revision", 101}, {"op_id", params.value("op_id")},
                    {"changed", QJsonObject{{"items", QJsonArray{}}, {"sessions", QJsonArray{}},
                                            {"projects", QJsonArray{}}, {"tags", QJsonArray{}},
                                            {"metrics", QJsonArray{}}, {"marks", QJsonArray{}},
                                            {"deleted", QJsonArray{}}}}});
                return;
            }
            const QString method = request.value("method").toString();
            if (method == "day.get") server->respond(socket, request, day);
            else if (method == "stats.get") server->respond(socket, request, fixture("stats"));
            else if (method == "config.get") server->respond(socket, request, QJsonObject{{"sleep_time", "23:00"}, {"location", "Tashkent"}, {"latitude", 41.2}, {"longitude", 69.2}, {"timezone_offset", 5}, {"method", "uzbekistan"}, {"asr", "hanafi"}, {"auto_location", true}, {"last_location_provider", QJsonValue::Null}, {"last_location_sync", QJsonValue::Null}});
            else server->respond(socket, request, QJsonObject{});
        });
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy spy(&backend, &Backend::requestFinished);
        backend.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 4, 3000);

        Theme theme(dir.path());
        QQmlEngine engine; engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl(QStringLiteral("qrc:/qml/Main.qml")));
        QScopedPointer<QObject> root(component.create());
        QVERIFY2(root, qPrintable(component.errorString()));
        auto *window = qobject_cast<QQuickWindow *>(root.data()); QVERIFY(window);
        window->show(); window->setWidth(1440); window->setHeight(1600);
        waitForBackendIdle(backend);
        auto *list = timelineView(root.data()); QVERIFY(list);
QTRY_VERIFY_WITH_TIMEOUT(itemDelegate(list, "<id:4>") != nullptr, 3000);
        auto *model = backend.dayTimeline();
        const QStringList before = timelineKeys(model);
const int original = model->indexOfItem("<id:4>");
        const int secondOriginal = model->indexOfItem("fajr-2");
        const int dhuhrBefore = rowForKey(model, "header:dhuhr");
        QVERIFY(original >= 0); QVERIFY(secondOriginal > original); QVERIFY(dhuhrBefore > secondOriginal);
auto *card = itemDelegate(list, "<id:4>"); QVERIFY(card);
        auto *handle = dragHandle(card); QVERIFY(handle);
        auto *target = rowDelegate(list, dhuhrBefore); QVERIFY(target);
        const QPoint targetPoint = sceneCenter(target);

        QTest::mousePress(window, Qt::LeftButton, Qt::NoModifier, sceneCenter(handle));
        QTRY_VERIFY_WITH_TIMEOUT(root->property("dragging").toBool(), 1000);
        QTest::mouseMove(window, targetPoint);
        QTRY_VERIFY_WITH_TIMEOUT(model->indexOfItem("<id:4>") != original, 1000);
        QTest::qWait(200);

        const int moved = model->indexOfItem("<id:4>");
        QCOMPARE(moved, rowForKey(model, "header:dhuhr") + 1);
        QCOMPARE(model->kindAt(moved), QString("item"));
        QCOMPARE(model->keyAt(moved + 1), QString("add:dhuhr"));
        QCOMPARE(model->rowCount(), before.size());
        int occurrences = 0;
        for (int row = 0; row < model->rowCount(); ++row)
            if (model->itemIdAt(row) == "<id:4>") ++occurrences;
        QCOMPARE(occurrences, 1);
        QCOMPARE(model->indexOfItem("fajr-2"), secondOriginal - 1);

        QTest::mouseRelease(window, Qt::LeftButton, Qt::NoModifier,
                            sceneCenter(rowDelegate(list, rowForKey(model, "header:dhuhr"))));
        QTRY_COMPARE_WITH_TIMEOUT(requestCount(daemon, "item.move"), 1, 3000);
        const QJsonObject params = lastRequest(daemon, "item.move").value("params").toObject();
        QCOMPARE(params.value("id").toString(), QString("<id:4>"));
        QCOMPARE(params.value("block").toString(), QString("dhuhr"));
        QCOMPARE(params.value("index").toInt(), 0);
        QVERIFY(!params.contains("after_id"));
        QVERIFY(!params.contains("before_id"));
        QTRY_COMPARE_WITH_TIMEOUT(model->blockAt(model->indexOfItem("<id:4>")), QString("dhuhr"), 3000);
    }

    void timelineDragWithinBlockSendsAfterAndBeforeAnchors() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        QJsonObject day = dragDay();
        daemon.setHandler([day](const QJsonObject &request, QLocalSocket *socket, FakeDaemon *server) mutable {
            if (request.value("method").toString() == "item.move") {
                const QJsonObject params = request.value("params").toObject();
                day = movedDay(day, params);
                server->respond(socket, request, QJsonObject{
                    {"revision", 101}, {"op_id", params.value("op_id")},
                    {"changed", QJsonObject{{"items", QJsonArray{}}, {"sessions", QJsonArray{}},
                                            {"projects", QJsonArray{}}, {"tags", QJsonArray{}},
                                            {"metrics", QJsonArray{}}, {"marks", QJsonArray{}},
                                            {"deleted", QJsonArray{}}}}});
                return;
            }
            const QString method = request.value("method").toString();
            if (method == "day.get") server->respond(socket, request, day);
            else if (method == "stats.get") server->respond(socket, request, fixture("stats"));
            else if (method == "config.get") server->respond(socket, request, QJsonObject{{"sleep_time", "23:00"}, {"location", "Tashkent"}, {"latitude", 41.2}, {"longitude", 69.2}, {"timezone_offset", 5}, {"method", "uzbekistan"}, {"asr", "hanafi"}, {"auto_location", true}, {"last_location_provider", QJsonValue::Null}, {"last_location_sync", QJsonValue::Null}});
            else server->respond(socket, request, QJsonObject{});
        });
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy spy(&backend, &Backend::requestFinished);
        backend.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 4, 3000);

        Theme theme(dir.path());
        QQmlEngine engine; engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl(QStringLiteral("qrc:/qml/Main.qml")));
        QScopedPointer<QObject> root(component.create());
        QVERIFY2(root, qPrintable(component.errorString()));
        auto *window = qobject_cast<QQuickWindow *>(root.data()); QVERIFY(window);
        window->show(); window->setWidth(1440); window->setHeight(1600);
        waitForBackendIdle(backend);
        auto *list = timelineView(root.data()); QVERIFY(list);
QTRY_VERIFY_WITH_TIMEOUT(itemDelegate(list, "<id:4>") != nullptr, 3000);
        auto *model = backend.dayTimeline();
        const int id1Before = model->indexOfItem("<id:4>");
        const int id2Before = model->indexOfItem("fajr-2");
        QVERIFY(id1Before >= 0); QVERIFY(id2Before > id1Before);

auto *card = itemDelegate(list, "<id:4>"); QVERIFY(card);
        auto *handle = dragHandle(card); QVERIFY(handle);
        auto *target = rowDelegate(list, id2Before); QVERIFY(target);
        QTest::mousePress(window, Qt::LeftButton, Qt::NoModifier, sceneCenter(handle));
        QTRY_VERIFY_WITH_TIMEOUT(root->property("dragging").toBool(), 1000);
        QTest::mouseMove(window, sceneCenter(target, 0.8));
        QTRY_VERIFY_WITH_TIMEOUT(model->indexOfItem("<id:4>") > rowForKey(model, "item:fajr-2"), 1000);
        QCOMPARE(model->indexOfItem("<id:4>"), rowForKey(model, "item:fajr-2") + 1);
        QTest::qWait(200);
        QTest::mouseRelease(window, Qt::LeftButton, Qt::NoModifier,
                            sceneCenter(rowDelegate(list, model->indexOfItem("<id:4>"))));
        QTRY_COMPARE_WITH_TIMEOUT(requestCount(daemon, "item.move"), 1, 3000);
        QJsonObject params = lastRequest(daemon, "item.move").value("params").toObject();
        QCOMPARE(params.value("id").toString(), QString("<id:4>"));
        QCOMPARE(params.value("block").toString(), QString("fajr"));
        QCOMPARE(params.value("after_id").toString(), QString("fajr-2"));
        QVERIFY(!params.contains("before_id"));
        QTest::qWait(100);

        card = itemDelegate(list, "<id:4>"); QVERIFY(card);
        handle = dragHandle(card); QVERIFY(handle);
        const int id1Now = model->indexOfItem("<id:4>");
        const int id2Now = model->indexOfItem("fajr-2");
        QCOMPARE(id2Now, id1Now - 1);
        QTest::mousePress(window, Qt::LeftButton, Qt::NoModifier, sceneCenter(handle));
        QTRY_VERIFY_WITH_TIMEOUT(root->property("dragging").toBool(), 1000);
        QTest::mouseMove(window, sceneCenter(rowDelegate(list, id2Now), 0.2));
        QTRY_VERIFY_WITH_TIMEOUT(model->indexOfItem("<id:4>") < rowForKey(model, "item:fajr-2"), 1000);
        QTest::qWait(200);
        QTest::mouseRelease(window, Qt::LeftButton, Qt::NoModifier,
                            sceneCenter(rowDelegate(list, model->indexOfItem("<id:4>"))));
        QTRY_COMPARE_WITH_TIMEOUT(requestCount(daemon, "item.move"), 2, 3000);
        params = lastRequest(daemon, "item.move").value("params").toObject();
        QCOMPARE(params.value("id").toString(), QString("<id:4>"));
        QCOMPARE(params.value("block").toString(), QString("fajr"));
        QCOMPARE(params.value("before_id").toString(), QString("fajr-2"));
        QVERIFY(!params.contains("after_id"));
    }

    void timelineDragEscapeCancelsAndRestoresOrder() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        daemon.setHandler(readHandler(dragDay()));
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy spy(&backend, &Backend::requestFinished);
        backend.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 4, 3000);

        Theme theme(dir.path());
        QQmlEngine engine; engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl(QStringLiteral("qrc:/qml/Main.qml")));
        QScopedPointer<QObject> root(component.create());
        QVERIFY2(root, qPrintable(component.errorString()));
        auto *window = qobject_cast<QQuickWindow *>(root.data()); QVERIFY(window);
        window->show(); window->setWidth(1440); window->setHeight(1600);
        window->requestActivate();
        waitForBackendIdle(backend);
        auto *list = timelineView(root.data()); QVERIFY(list);
QTRY_VERIFY_WITH_TIMEOUT(itemDelegate(list, "<id:5>") != nullptr, 3000);
        auto *model = backend.dayTimeline();
        const QStringList before = timelineKeys(model);
const int original = model->indexOfItem("<id:5>");
const int targetRow = model->indexOfItem("<id:4>");
        QVERIFY(original > targetRow);

auto *card = itemDelegate(list, "<id:5>"); QVERIFY(card);
        auto *handle = dragHandle(card); QVERIFY(handle);
        QTest::mousePress(window, Qt::LeftButton, Qt::NoModifier, sceneCenter(handle));
        QTRY_VERIFY_WITH_TIMEOUT(root->property("dragging").toBool(), 1000);
        QTest::mouseMove(window, sceneCenter(rowDelegate(list, targetRow)));
        QTRY_VERIFY_WITH_TIMEOUT(model->indexOfItem("<id:5>") != original, 1000);
        QTest::qWait(200);

        QTest::keyClick(window, Qt::Key_Escape);
        QTRY_VERIFY_WITH_TIMEOUT(!root->property("dragging").toBool(), 3000);
        QCOMPARE(timelineKeys(model), before);
        QTest::mouseRelease(window, Qt::LeftButton, Qt::NoModifier,
                            sceneCenter(rowDelegate(list, targetRow)));
        QTest::qWait(100);
        QCOMPARE(requestCount(daemon, "item.move"), 0);
    }

    void timelineDragFailedMoveRestoresServerOrder() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        const QJsonObject day = dragDay();
        auto reads = readHandler(day);
        daemon.setHandler([reads](const QJsonObject &request, QLocalSocket *socket, FakeDaemon *server) {
            if (request.value("method").toString() == "item.move") {
                server->respondError(socket, request, -32000, "test", "move rejected");
                return;
            }
            reads(request, socket, server);
        });
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy spy(&backend, &Backend::requestFinished);
        backend.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 4, 3000);

        Theme theme(dir.path());
        QQmlEngine engine; engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl(QStringLiteral("qrc:/qml/Main.qml")));
        QScopedPointer<QObject> root(component.create());
        QVERIFY2(root, qPrintable(component.errorString()));
        auto *window = qobject_cast<QQuickWindow *>(root.data()); QVERIFY(window);
        window->show(); window->setWidth(1440); window->setHeight(1600);
        waitForBackendIdle(backend);
        auto *list = timelineView(root.data()); QVERIFY(list);
QTRY_VERIFY_WITH_TIMEOUT(itemDelegate(list, "<id:4>") != nullptr, 3000);
        auto *model = backend.dayTimeline();
        const QStringList before = timelineKeys(model);
const int original = model->indexOfItem("<id:4>");
        const int dhuhrRow = rowForKey(model, "header:dhuhr");
        QVERIFY(original >= 0); QVERIFY(dhuhrRow > original);

auto *card = itemDelegate(list, "<id:4>"); QVERIFY(card);
        auto *handle = dragHandle(card); QVERIFY(handle);
        QTest::mousePress(window, Qt::LeftButton, Qt::NoModifier, sceneCenter(handle));
        QTRY_VERIFY_WITH_TIMEOUT(root->property("dragging").toBool(), 1000);
        QTest::mouseMove(window, sceneCenter(rowDelegate(list, dhuhrRow)));
        QTRY_VERIFY_WITH_TIMEOUT(model->indexOfItem("<id:4>") != original, 1000);
        QTest::qWait(200);
        QTest::mouseRelease(window, Qt::LeftButton, Qt::NoModifier,
                            sceneCenter(rowDelegate(list, rowForKey(model, "header:dhuhr"))));
        QTRY_COMPARE_WITH_TIMEOUT(requestCount(daemon, "item.move"), 1, 3000);
        QTRY_VERIFY_WITH_TIMEOUT(backend.error().contains("move rejected"), 3000);
        QTRY_COMPARE_WITH_TIMEOUT(timelineKeys(model), before, 3000);
QCOMPARE(model->indexOfItem("<id:4>"), original);
    }

    void timelineDragClampsAboveAndBelowTheBlocks() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        daemon.setHandler(readHandler(dragDay()));
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy spy(&backend, &Backend::requestFinished);
        backend.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 4, 3000);

        Theme theme(dir.path());
        QQmlEngine engine; engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl(QStringLiteral("qrc:/qml/Main.qml")));
        QScopedPointer<QObject> root(component.create());
        QVERIFY2(root, qPrintable(component.errorString()));
        auto *window = qobject_cast<QQuickWindow *>(root.data()); QVERIFY(window);
        window->show(); window->setWidth(1440); window->setHeight(1600);
        window->requestActivate();
        waitForBackendIdle(backend);
        auto *list = timelineView(root.data()); QVERIFY(list);
QTRY_VERIFY_WITH_TIMEOUT(itemDelegate(list, "<id:4>") != nullptr, 3000);
        auto *model = backend.dayTimeline();
        const QStringList before = timelineKeys(model);
const int original = model->indexOfItem("<id:4>");
        QVERIFY(original >= 0);

        auto *card = itemDelegate(list, "<id:4>"); QVERIFY(card);
        auto *handle = dragHandle(card); QVERIFY(handle);
        QTest::mousePress(window, Qt::LeftButton, Qt::NoModifier, sceneCenter(handle));
        QTRY_VERIFY_WITH_TIMEOUT(root->property("dragging").toBool(), 1000);

        QTest::mouseMove(window, list->mapToScene(QPointF(10, -20)).toPoint());
        QTest::qWait(200);
        QTRY_COMPARE_WITH_TIMEOUT(model->indexOfItem("<id:4>"),
                                  rowForKey(model, "header:night") + 1, 1000);
        const int clampedAbove = model->indexOfItem("<id:4>");
        QCOMPARE(model->kindAt(clampedAbove), QString("item"));

        const int sunrise = rowForKey(model, "marker:sunrise");
        QVERIFY(sunrise >= 0);
        QTest::mouseMove(window, sceneCenter(rowDelegate(list, sunrise)));
        QTest::qWait(200);
        QCOMPARE(model->indexOfItem("<id:4>"), clampedAbove);
        QVERIFY(model->kindAt(clampedAbove) != QString("marker"));

        QTest::mouseMove(window, list->mapToScene(QPointF(10, list->height() - 1)).toPoint());
        QTest::qWait(200);
        QTRY_COMPARE_WITH_TIMEOUT(model->indexOfItem("<id:4>"),
                                  rowForKey(model, "add:isha") - 1, 1000);
        const int clampedBelow = model->indexOfItem("<id:4>");
        const int addIsha = rowForKey(model, "add:isha");
        QVERIFY(addIsha >= 0);
        QCOMPARE(clampedBelow, addIsha - 1);
        QCOMPARE(model->keyAt(clampedBelow + 1), QString("add:isha"));
        const int sleep = rowForKey(model, "marker:sleep");
        QVERIFY(sleep > addIsha);
        QTest::mouseMove(window, sceneCenter(rowDelegate(list, sleep)));
        QTest::qWait(200);
        QCOMPARE(model->indexOfItem("<id:4>"), clampedBelow);
        QCOMPARE(model->keyAt(clampedBelow + 1), QString("add:isha"));

        QTest::keyClick(window, Qt::Key_Escape);
        QTRY_VERIFY_WITH_TIMEOUT(!root->property("dragging").toBool(), 3000);
        QCOMPARE(timelineKeys(model), before);
        QTest::mouseRelease(window, Qt::LeftButton, Qt::NoModifier,
                            sceneCenter(rowDelegate(list, original)));
        QTest::qWait(100);
        QCOMPARE(requestCount(daemon, "item.move"), 0);
    }

    void dayTimelineModelDiffsRowsInPlace() {
        DayTimelineModel model;
        QAbstractItemModelTester tester(&model, QAbstractItemModelTester::FailureReportingMode::QtTest);
        auto make = [](DayTimelineModel::Kind kind, const QString &key, const QString &title) {
            DayTimelineModel::Row row;
            row.kind = kind;
            row.key = key;
            row.block = "fajr";
            row.itemId = key;
            row.title = title;
            row.state = "open";
            row.measure = "check";
            row.repeat = "daily";
            return row;
        };
        QVector<DayTimelineModel::Row> rows{
            make(DayTimelineModel::Kind::Header, "header:fajr", ""),
            make(DayTimelineModel::Kind::Item, "item:a", "A"),
            make(DayTimelineModel::Kind::Item, "item:b", "B"),
            make(DayTimelineModel::Kind::Add, "add:fajr", "")};
        model.setRows(rows);
        QCOMPARE(model.rowCount(), 4);

        QSignalSpy changed(&model, &QAbstractItemModel::dataChanged);
        QSignalSpy inserted(&model, &QAbstractItemModel::rowsInserted);
        QSignalSpy removed(&model, &QAbstractItemModel::rowsRemoved);
        QSignalSpy moved(&model, &QAbstractItemModel::rowsMoved);
        QSignalSpy reset(&model, &QAbstractItemModel::modelReset);

        rows[1].title = "A changed";
        model.setRows(rows);
        QCOMPARE(changed.size(), 1);
        const QList<QVariant> change = changed.takeFirst();
        QCOMPARE(change.at(0).toModelIndex().row(), 1);
        QCOMPARE(change.at(1).toModelIndex().row(), 1);
        QVERIFY(change.at(2).value<QList<int>>().contains(DayTimelineModel::TitleRole));

        rows.insert(3, make(DayTimelineModel::Kind::Item, "item:c", "C"));
        model.setRows(rows);
        QCOMPARE(inserted.size(), 1);
        QCOMPARE(inserted.takeFirst().at(1).toInt(), 3);

        rows.move(3, 1);
        model.setRows(rows);
        QCOMPARE(moved.size(), 1);

        rows.removeAt(1);
        model.setRows(rows);
        QCOMPARE(removed.size(), 1);
        QCOMPARE(reset.size(), 0);
        QCOMPARE(model.indexOfItem("b") >= 0, true);
    }

    void prayerSeriesDailyAndStatsModelsReportDiffs() {
        PrayerModel prayers;
        QAbstractItemModelTester prayerTester(&prayers, QAbstractItemModelTester::FailureReportingMode::QtTest);
        QVector<PrayerModel::Row> prayerRows;
        for (const QString &id : {"fajr", "dhuhr", "asr", "maghrib", "isha"}) {
            PrayerModel::Row row;
            row.prayerId = id;
            row.name = id;
            row.time = "05:00";
            prayerRows.append(row);
        }
        prayers.setRows(prayerRows);
        QSignalSpy prayerChanged(&prayers, &QAbstractItemModel::dataChanged);
        prayerRows[0].marked = true;
        prayers.setRows(prayerRows);
        QCOMPARE(prayerChanged.size(), 1);
        QCOMPARE(prayerChanged.takeFirst().at(0).toModelIndex().row(), 0);

        StatsSeriesModel series;
        QAbstractItemModelTester seriesTester(&series, QAbstractItemModelTester::FailureReportingMode::QtTest);
        series.setRows({{"2031-05-14", true, true, 25, 0}, {"2031-05-15", true, false, 0, 0}});
        QCOMPARE(series.rowCount(), 2);

        const auto stats = Dto::decode<Dto::Stats>(fixture("stats_days"));
        QVERIFY(stats);
        StatsDailyModel daily;
        QAbstractItemModelTester dailyTester(&daily, QAbstractItemModelTester::FailureReportingMode::QtTest);
        daily.setStats(*stats.value, "9999-12-31", "");
        QVERIFY(daily.rowCount() > 0);
        QSignalSpy dailyChanged(&daily, &QAbstractItemModel::dataChanged);
        QSignalSpy dailyReset(&daily, &QAbstractItemModel::modelReset);
        Dto::Stats changedStats = *stats.value;
        changedStats.daily[0].json["minutes"] = changedStats.daily[0].json.value("minutes").toInt() + 7;
        daily.setStats(changedStats, "9999-12-31", "");
        QCOMPARE(dailyChanged.size(), 1);
        QCOMPARE(dailyChanged.takeFirst().at(0).toModelIndex().row(), 0);
        QCOMPARE(dailyReset.size(), 0);

        StatsItemsModel items;
        QAbstractItemModelTester itemsTester(&items, QAbstractItemModelTester::FailureReportingMode::QtTest);
        items.setItems(stats.value->items);
        QVERIFY(items.rowCount() > 0);
        QVERIFY(!items.data(items.index(0, 0), StatsItemsModel::TitleRole).toString().isEmpty());

        RoutinesModel routines;
        QAbstractItemModelTester routinesTester(&routines, QAbstractItemModelTester::FailureReportingMode::QtTest);
        routines.setItems(rpcFixture("items_list").value("result").toArray().toVariantList());
        QCOMPARE(routines.rowCount(), 1);
        QCOMPARE(routines.data(routines.index(0, 0), RoutinesModel::RoutineRole).toMap().value("repeat"), "daily");

        QCOMPARE(stats.value->json.value("series").toArray().first().toObject().value("metric_id").toString(), QString("minutes"));
    }

    void timelineOrdersItemsByBlockAndPosition() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        QJsonObject day = fixture("day");
        QJsonArray blocks = day.value("blocks").toArray();
        for (int b = 0; b < blocks.size(); ++b) {
            QJsonObject block = blocks[b].toObject();
            if (block.value("start").toString() != "fajr") continue;
            QJsonObject base = fixture("day").value("blocks").toArray().at(b).toObject()
                .value("items").toArray().first().toObject();
            QJsonArray ordered;
            for (int i = 0; i < 3; ++i) {
                QJsonObject item = base;
                item["id"] = QStringLiteral("fajr-%1").arg(i);
                item["title"] = QStringLiteral("Fajr %1").arg(i);
                item["position"] = i == 0 ? 2 : i == 1 ? 0 : 1;
                ordered.append(item);
            }
            block["items"] = ordered;
            blocks[b] = block;
            break;
        }
        day["blocks"] = blocks;
        daemon.setHandler(readHandler(day));
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy spy(&backend, &Backend::requestFinished);
        backend.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 4, 3000);
        QStringList ids;
        for (int row = 0; row < backend.dayTimeline()->rowCount(); ++row) {
            if (backend.dayTimeline()->data(backend.dayTimeline()->index(row, 0),
                                            DayTimelineModel::BlockRole).toString() != "fajr")
                continue;
            if (backend.dayTimeline()->data(backend.dayTimeline()->index(row, 0),
                                            DayTimelineModel::RowKindRole).toString() != "item")
                continue;
            ids << backend.dayTimeline()->data(backend.dayTimeline()->index(row, 0),
                                               DayTimelineModel::ItemIdRole).toString();
        }
        QCOMPARE(ids, QStringList({"fajr-1", "fajr-2", "fajr-0"}));
    }

    void singleRowMutationChangesOnlyThatRow() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        QJsonObject day = fixture("day");
        QJsonObject edited;
        for (const QJsonValue &blockValue : day.value("blocks").toArray()) {
            for (const QJsonValue &itemValue : blockValue.toObject().value("items").toArray()) {
                if (itemValue.toObject().value("id").toString() == "<id:4>") edited = itemValue.toObject();
            }
        }
        QVERIFY(!edited.isEmpty());
        daemon.setHandler([day, edited](const QJsonObject &request, QLocalSocket *socket, FakeDaemon *server) mutable {
            if (request.value("method").toString() == "item.edit") {
                QJsonObject row = edited;
                row["title"] = "Mutated";
                row["version"] = 8;
                QJsonArray blocks = day.value("blocks").toArray();
                for (int b = 0; b < blocks.size(); ++b) {
                    QJsonObject block = blocks[b].toObject();
                    QJsonArray rows = block.value("items").toArray();
                    for (int i = 0; i < rows.size(); ++i) {
                        if (rows[i].toObject().value("id").toString() != "<id:4>") continue;
                        rows[i] = row; block["items"] = rows; blocks[b] = block;
                    }
                }
                day["blocks"] = blocks;
                QJsonObject itemView = fixture("item_edit").value("changed").toObject().value("items").toArray().first().toObject();
                itemView["id"] = "<id:4>"; itemView["title"] = "Mutated"; itemView["version"] = 8;
                const QJsonObject changed{{"items", QJsonArray{itemView}}, {"sessions", QJsonArray{}},
                                          {"projects", QJsonArray{}}, {"tags", QJsonArray{}},
                                          {"metrics", QJsonArray{}}, {"marks", QJsonArray{}},
                                          {"deleted", QJsonArray{}}};
                server->respond(socket, request, QJsonObject{
                    {"revision", 101}, {"op_id", request.value("params").toObject().value("op_id")},
                    {"changed", changed}});
                return;
            }
            const QString method = request.value("method").toString();
            if (method == "day.get") server->respond(socket, request, day);
            else if (method == "stats.get") server->respond(socket, request, fixture("stats"));
            else if (method == "config.get") server->respond(socket, request, QJsonObject{{"sleep_time", "23:00"}, {"location", "Tashkent"}, {"latitude", 41.2}, {"longitude", 69.2}, {"timezone_offset", 5}, {"method", "uzbekistan"}, {"asr", "hanafi"}, {"auto_location", true}, {"last_location_provider", QJsonValue::Null}, {"last_location_sync", QJsonValue::Null}});
            else server->respond(socket, request, QJsonObject{});
        });
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy spy(&backend, &Backend::requestFinished);
        backend.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 4, 3000);
        QSignalSpy changed(backend.dayTimeline(), &QAbstractItemModel::dataChanged);
        QSignalSpy reset(backend.dayTimeline(), &QAbstractItemModel::modelReset);
        QVariantMap item = itemEditMap(backend, "<id:4>");
        item["title"] = "Mutated";
        backend.saveItem(item);
        QTRY_VERIFY_WITH_TIMEOUT(timelineItem(backend.dayTimeline(), "<id:4>") >= 0, 3000);
        QTRY_VERIFY_WITH_TIMEOUT(backend.dayTimeline()->data(
            backend.dayTimeline()->index(timelineItem(backend.dayTimeline(), "<id:4>"), 0),
            DayTimelineModel::TitleRole).toString() == "Mutated", 3000);
        QVERIFY(changed.size() >= 1);
        for (const QList<QVariant> &args : changed) {
            QCOMPARE(args.at(0).toModelIndex().row(), args.at(1).toModelIndex().row());
        }
        QCOMPARE(reset.size(), 0);
    }

    void settingsListsAreSeparateAndEntitySavesSendFullFields() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        daemon.setHandler(trackingHandler(trackingState()));
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy settings(&backend, &Backend::settingsChanged);
        backend.loadSettings();
        // The three lists used to share one coalescing key and all received
        // the projects list.
        QTRY_COMPARE_WITH_TIMEOUT(settings.size(), 4, 3000);
        QCOMPARE(backend.projects().size(), 4);
        QCOMPARE(backend.epics().size(), 5);
        QCOMPARE(backend.tags().size(), 3);
        QCOMPARE(backend.metrics().size(), 7);
        QCOMPARE(backend.tags().first().toMap().value("name").toString(), QString("Faith"));
        QCOMPARE(backend.metrics().first().toMap().value("id").toString(), QString("focus"));

        QSignalSpy saved(&backend, &Backend::entitySaved);
        backend.saveEntity("project", QVariantMap{
            {"name", "Reading"}, {"color", "#26a69a"}, {"note", ""},
            {"tag_ids", QVariantList{"learning"}},
            {"metrics", QVariantList{QVariantMap{{"metric_id", "focus"}, {"required", true}},
                                     QVariantMap{{"metric_id", "pages"}, {"required", false}}}}});
        QTRY_COMPARE_WITH_TIMEOUT(saved.size(), 1, 3000);
        QCOMPARE(saved.first().at(0).toString(), QString("project"));
        QCOMPARE(saved.first().at(1).toString(), QString("new-project"));
        const QJsonObject add = lastRequest(daemon, "project.add").value("params").toObject();
        QCOMPARE(add.value("name").toString(), QString("Reading"));
        QCOMPARE(add.value("tag_ids").toArray(), QJsonArray{"learning"});
        const QJsonArray metrics = add.value("metrics").toArray();
        QCOMPARE(metrics.size(), 2);
        QCOMPARE(metrics.at(0).toObject().value("metric_id").toString(), QString("focus"));
        QCOMPARE(metrics.at(0).toObject().value("required").toBool(), true);
        QCOMPARE(metrics.at(1).toObject().value("required").toBool(), false);
        QVERIFY(!add.value("op_id").toString().isEmpty());

        backend.saveEntity("project", QVariantMap{{"id", "quran"}, {"archived", true}});
        QTRY_COMPARE_WITH_TIMEOUT(saved.size(), 2, 3000);
        const QJsonObject archive = lastRequest(daemon, "project.edit").value("params").toObject();
        QCOMPARE(archive.value("id").toString(), QString("quran"));
        QCOMPARE(archive.value("archived").toBool(), true);

        backend.saveEntity("metric", QVariantMap{{"name", "Verses"}, {"unit", "verses"}, {"kind", "count"}, {"rollup", "sum"}});
        QTRY_COMPARE_WITH_TIMEOUT(saved.size(), 3, 3000);
        QCOMPARE(lastRequest(daemon, "metric.add").value("params").toObject().value("kind").toString(), QString("count"));

        backend.removeTag("health");
        QTRY_COMPARE_WITH_TIMEOUT(saved.size(), 4, 3000);
        QCOMPARE(lastRequest(daemon, "tag.remove").value("params").toObject().value("id").toString(), QString("health"));
        QCOMPARE(saved.last().at(1).toString(), QString());
        QVERIFY(backend.error().isEmpty());
    }
    void itemEditorPicksAndCreatesProjects() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        daemon.setHandler(trackingHandler(trackingState()));
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy spy(&backend, &Backend::requestFinished);
        backend.reload(); QTRY_VERIFY_WITH_TIMEOUT(spy.size() >= 3, 3000);
        ItemDraft *draft = backend.itemForId("quran");
        QCOMPARE(draft->projectId(), QString("quran"));
        QCOMPARE(draft->plannedMinutes(), 30);
        Theme theme(dir.path());
        QQmlEngine engine; engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl::fromLocalFile(QStringLiteral(TEST_SOURCE_DIR "/../src/qml/Main.qml")));
        QScopedPointer<QObject> root(component.create()); QVERIFY2(root, qPrintable(component.errorString()));
        root->setProperty("visible", false);
        QTRY_COMPARE_WITH_TIMEOUT(backend.projects().size(), 4, 3000);
        QVERIFY(QMetaObject::invokeMethod(root.data(), "openEntry", Q_ARG(QVariant, QVariant("quran"))));
        QObject *entry = root->findChild<QObject *>("entryPopup"); QVERIFY(entry);
        QTRY_COMPARE_WITH_TIMEOUT(entry->property("projectId").toString(), QString("quran"), 3000);
        // Archived projects are not offered.
        auto *entryContent = entry->property("contentItem").value<QQuickItem *>(); QVERIFY(entryContent);
        auto entryHasText = [&](const QString &text) {
            for (QQuickItem *item : quickDescendants(entryContent))
                if (item->property("text").toString() == text) return true;
            return false;
        };
        QTRY_VERIFY_WITH_TIMEOUT(entryHasText("Study"), 3000);
        QVERIFY(entryHasText("No project"));
        QVERIFY(!entryHasText("Garden"));
        QObject *field = findByName(root.data(), "newProjectField"); QVERIFY(field);
        field->setProperty("text", "Reading");
        QVERIFY(QMetaObject::invokeMethod(field, "accepted"));
        QTRY_COMPARE_WITH_TIMEOUT(entry->property("projectId").toString(), QString("new-project"), 3000);
        QCOMPARE(lastRequest(daemon, "project.add").value("params").toObject().value("name").toString(), QString("Reading"));
        QVariant draftMap; QVERIFY(QMetaObject::invokeMethod(entry, "draft", Q_RETURN_ARG(QVariant, draftMap)));
        QCOMPARE(draftMap.toMap().value("project_id").toString(), QString("new-project"));
    }
    void tickUntickNeverLogsASecondTick() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        daemon.setHandler(trackingHandler(trackingState()));
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy spy(&backend, &Backend::requestFinished);
        backend.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 4, 3000);
        auto row = [&](const QString &id) { return timelineItem(backend.dayTimeline(), id); };
        auto role = [&](const QString &id, int r) { return backend.dayTimeline()->data(backend.dayTimeline()->index(row(id), 0), r); };
        QVERIFY(role("walk", DayTimelineModel::TickedRole).toBool());
        QVERIFY(role("quran", DayTimelineModel::TickedRole).toBool());
        // Timed sessions alone never make an item done.
        QVERIFY(!role("deep", DayTimelineModel::TickedRole).toBool());
        QCOMPARE(role("deep", DayTimelineModel::StateRole).toString(), QString("open"));
        QCOMPARE(role("quran", DayTimelineModel::SessionListRole).toList().size(), 3);
        QCOMPARE(role("quran", DayTimelineModel::ProjectNameRole).toString(), QString("Quran"));

        // Untick removes only the tick, never the timed sessions.
        QVERIFY(backend.toggleDone("quran"));
        QTRY_COMPARE_WITH_TIMEOUT(requestCount(daemon, "session.remove"), 1, 3000);
        QCOMPARE(lastRequest(daemon, "session.remove").value("params").toObject().value("id").toString(), QString("s-q3"));
        QVERIFY(backend.toggleDone("walk"));
        QTRY_COMPARE_WITH_TIMEOUT(requestCount(daemon, "session.remove"), 2, 3000);
        QCOMPARE(lastRequest(daemon, "session.remove").value("params").toObject().value("id").toString(), QString("s-w1"));
        QCOMPARE(requestCount(daemon, "session.log"), 0);

        // Not done: one tick without minutes, carrying the panel's values.
        backend.tickItem("dhikr", QVariantMap{{"focus", 5}}, "After salah");
        QTRY_COMPARE_WITH_TIMEOUT(requestCount(daemon, "session.log"), 1, 3000);
        const QJsonObject tick = lastRequest(daemon, "session.log").value("params").toObject();
        QCOMPARE(tick.value("item_id").toString(), QString("dhikr"));
        QVERIFY(!tick.contains("minutes"));
        QCOMPARE(tick.value("values").toObject().value("focus").toDouble(), 5.0);
        QCOMPARE(tick.value("note").toString(), QString("After salah"));

        // session.log stops a matching timer in the same write as the tick.
        backend.tickItem("deep", QVariantMap{{"focus", 4}});
        QTRY_COMPARE_WITH_TIMEOUT(requestCount(daemon, "session.log"), 2, 3000);
        QCOMPARE(requestCount(daemon, "session.stop"), 0);
        QCOMPARE(lastRequest(daemon, "session.log").value("params").toObject().value("item_id"), "deep");

        // Already done: tickItem never adds a second tick.
        waitForBackendIdle(backend);
        const int writes = daemon.requests.size();
        backend.tickItem("quran");
        backend.tickItem("walk");
        QTest::qWait(150);
        int laterWrites = 0;
        for (int i = writes; i < daemon.requests.size(); ++i)
            if (daemon.requests.at(i).value("params").toObject().contains("op_id")) ++laterWrites;
        QCOMPARE(laterWrites, 0);
    }
    void sessionWritesSendMinutesValuesAndNotes() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        auto state = trackingState();
        daemon.setHandler(trackingHandler(state));
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy spy(&backend, &Backend::requestFinished);
        QSignalSpy finished(&backend, &Backend::sessionWriteFinished);
        backend.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 4, 3000);

        backend.logSession("gym", {}, 30, QVariantMap{{"reps", 20}}, "Felt strong");
        QTRY_COMPARE_WITH_TIMEOUT(finished.size(), 1, 3000);
        QVERIFY(finished.last().at(0).toBool());
        const QJsonObject log = lastRequest(daemon, "session.log").value("params").toObject();
        QCOMPARE(log.value("item_id").toString(), QString("gym"));
        QCOMPARE(log.value("minutes").toInt(), 30);
        QCOMPARE(log.value("values").toObject().value("reps").toDouble(), 20.0);
        QCOMPARE(log.value("note").toString(), QString("Felt strong"));

        backend.logSession({}, "fitness", 15);
        QTRY_COMPARE_WITH_TIMEOUT(finished.size(), 2, 3000);
        const QJsonObject projectLog = lastRequest(daemon, "session.log").value("params").toObject();
        QVERIFY(!projectLog.contains("item_id"));
        QCOMPARE(projectLog.value("project_id").toString(), QString("fitness"));

        backend.editSession("s-q1", 40, QVariantMap{{"focus", 3}, {"quality", 4}}, "");
        QTRY_COMPARE_WITH_TIMEOUT(finished.size(), 3, 3000);
        const QJsonObject edit = lastRequest(daemon, "session.edit").value("params").toObject();
        QCOMPARE(edit.value("id").toString(), QString("s-q1"));
        QCOMPARE(edit.value("minutes").toInt(), 40);
        QCOMPARE(edit.value("values").toObject().value("quality").toDouble(), 4.0);
        QVERIFY(edit.value("note").isNull());

        backend.removeSession("s-q2");
        QTRY_COMPARE_WITH_TIMEOUT(finished.size(), 4, 3000);
        QCOMPARE(lastRequest(daemon, "session.remove").value("params").toObject().value("id").toString(), QString("s-q2"));

        // Saving a pause without edits keeps these fields optional.
        backend.stopSession("s-run");
        QTRY_COMPARE_WITH_TIMEOUT(finished.size(), 5, 3000);
        QVERIFY(finished.last().at(0).toBool());
        const QJsonObject stop = lastRequest(daemon, "session.stop").value("params").toObject();
        QCOMPARE(stop.value("id").toString(), QString("s-run"));
        QVERIFY(!stop.contains("values"));
        QVERIFY(!stop.contains("note"));
        QVERIFY(!stop.contains("minutes"));
        QVERIFY(!stop.contains("done"));
        QVERIFY(backend.error().isEmpty());
    }
    void pausePanelSendsOnlyChanges() {
        QTemporaryDir dir;
        FakeDaemon daemon(dir.filePath("salah.sock"));
        if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        auto state = trackingState();
        QJsonObject running = state->status.value("running").toObject();
        running["values"] = QJsonObject{{"quality", 4}};
        state->status["running"] = running;
        daemon.setHandler(trackingHandler(state));

        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        Theme theme(dir.path());
        QQmlEngine engine;
        engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl::fromLocalFile(QStringLiteral(TEST_SOURCE_DIR "/../src/qml/Main.qml")));
        QScopedPointer<QObject> root(component.create());
        QVERIFY2(root, qPrintable(component.errorString()));
        backend.reload();
        QTRY_COMPARE_WITH_TIMEOUT(backend.running().value("id").toString(), QString("s-run"), 3000);

        QObject *pause = findByName(root.data(), "runningStopButton");
        QObject *prompt = findByName(root.data(), "sessionPrompt");
        QObject *save = findByName(root.data(), "sessionSaveButton");
        QObject *saveDone = findByName(root.data(), "sessionSaveDoneButton");
        QObject *worked = findByName(root.data(), "sessionMinutesField");
        QObject *note = findByName(root.data(), "sessionNoteField");
        QVERIFY(pause && prompt && save && saveDone && worked && note);
        QVERIFY(QMetaObject::invokeMethod(pause, "clicked"));
        QTRY_VERIFY(prompt->property("visible").toBool());
        QCOMPARE(requestCount(daemon, "session.stop"), 0);
        QVERIFY(save->property("enabled").toBool());
        QVERIFY(saveDone->property("visible").toBool());

        QVERIFY(QMetaObject::invokeMethod(save, "clicked"));
        QTRY_VERIFY(!prompt->property("visible").toBool());
        QJsonObject params = lastRequest(daemon, "session.stop").value("params").toObject();
        QCOMPARE(params.value("id").toString(), QString("s-run"));
        QVERIFY(!params.value("op_id").toString().isEmpty());
        QVERIFY(!params.contains("minutes"));
        QVERIFY(!params.contains("values"));
        QVERIFY(!params.contains("note"));
        QVERIFY(!params.contains("done"));

        QVERIFY(QMetaObject::invokeMethod(pause, "clicked"));
        QTRY_VERIFY(prompt->property("visible").toBool());
        worked->setProperty("text", "26");
        QVERIFY(QMetaObject::invokeMethod(save, "clicked"));
        QTRY_VERIFY(!prompt->property("visible").toBool());
        params = lastRequest(daemon, "session.stop").value("params").toObject();
        QCOMPARE(params.value("minutes").toInt(), 26);
        QVERIFY(!params.contains("done"));

        QVERIFY(QMetaObject::invokeMethod(pause, "clicked"));
        QTRY_VERIFY(prompt->property("visible").toBool());
        worked->setProperty("text", "32");
        note->setProperty("text", "  Finished chapter  ");
        QVERIFY(QMetaObject::invokeMethod(prompt, "setValue", Q_ARG(QVariant, QVariant("focus")),
                                          Q_ARG(QVariant, QVariant(5))));
        QVERIFY(QMetaObject::invokeMethod(saveDone, "clicked"));
        QTRY_VERIFY(!prompt->property("visible").toBool());
        params = lastRequest(daemon, "session.stop").value("params").toObject();
        QCOMPARE(params.value("id").toString(), QString("s-run"));
        QCOMPARE(params.value("minutes").toInt(), 32);
        QCOMPARE(params.value("values").toObject(), (QJsonObject{{"focus", 5}}));
        QCOMPARE(params.value("note").toString(), QString("Finished chapter"));
        QCOMPARE(params.value("done"), QJsonValue(true));
        QCOMPARE(requestCount(daemon, "session.stop"), 3);
        QCOMPARE(requestCount(daemon, "session.log"), 0);
    }
    void pausePanelWorkedChipsAndProjectOnlyActions() {
        QTemporaryDir dir;
        Backend backend(nullptr, fake(dir), 3000, daemon_path(dir));
        Theme theme(dir.path());
        QQmlEngine engine;
        engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl::fromLocalFile(QStringLiteral(TEST_SOURCE_DIR "/../src/qml/Main.qml")));
        QScopedPointer<QObject> root(component.create());
        QVERIFY2(root, qPrintable(component.errorString()));
        QObject *prompt = findByName(root.data(), "sessionPrompt");
        QObject *save = findByName(root.data(), "sessionSaveButton");
        QObject *saveDone = findByName(root.data(), "sessionSaveDoneButton");
        QObject *worked = findByName(root.data(), "sessionMinutesField");
        QVERIFY(prompt && save && saveDone && worked);

        QVariantMap options{{"mode", "pause"}, {"sessionId", "s-run"}, {"itemId", "deep"},
                            {"title", "Deep work: grammar"}, {"color", "#9b59b6"}, {"minutes", 47},
                            {"metrics", TrackingFixture::promptFor("study").toVariantList()}};
        QVERIFY(QMetaObject::invokeMethod(prompt, "show", Q_ARG(QVariant, QVariant(options))));
        QTRY_VERIFY(prompt->property("visible").toBool());
        QCOMPARE(worked->property("text").toString(), QString("47"));
        QVERIFY(hasLabelText(prompt, "Worked"));
        QVERIFY(save->property("enabled").toBool());
        QVERIFY(saveDone->property("enabled").toBool());
        QVERIFY(saveDone->property("visible").toBool());
        auto *window = qobject_cast<QQuickWindow *>(root.data());
        QVERIFY(window);
        if (!qEnvironmentVariableIsEmpty("SALAH_UI_SHOT_DIR"))
            shotWindow(window, "prompt-pause-preview.png");
        auto *content = prompt->property("contentItem").value<QQuickItem *>();
        QVERIFY(content);
        int remaining = 47;
        for (int amount : {5, 15, 30}) {
            QQuickItem *chip = nullptr;
            for (QQuickItem *item : quickDescendants(content)) {
                if (item->objectName() == QStringLiteral("subtractWorked%1").arg(amount)) {
                    chip = item;
                    break;
                }
            }
            QVERIFY(chip);
            QVERIFY(QMetaObject::invokeMethod(chip, "clicked"));
            remaining = qMax(1, remaining - amount);
            QCOMPARE(worked->property("text").toInt(), remaining);
        }
        worked->setProperty("text", "");
        QVERIFY(save->property("enabled").toBool());
        QVERIFY(saveDone->property("enabled").toBool());

        options.remove("itemId");
        options["title"] = "Study";
        QVERIFY(QMetaObject::invokeMethod(prompt, "show", Q_ARG(QVariant, QVariant(options))));
        QVERIFY(!saveDone->property("visible").toBool());
        QVERIFY(save->property("enabled").toBool());
    }
    void runningSessionComesFromStatus() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        auto state = trackingState();
        daemon.setHandler(trackingHandler(state));
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy spy(&backend, &Backend::requestFinished);
        backend.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 4, 3000);
        QCOMPARE(backend.running().value("id").toString(), QString("s-run"));
        QCOMPARE(backend.runningTitle(), QString("Deep work: grammar"));
        QCOMPARE(backend.runningColor(), QString("#9b59b6"));
        const int deep = timelineItem(backend.dayTimeline(), "deep");
        QVERIFY(backend.dayTimeline()->data(backend.dayTimeline()->index(deep, 0), DayTimelineModel::TimerRunningRole).toBool());
        QCOMPARE(backend.dayTimeline()->data(backend.dayTimeline()->index(deep, 0), DayTimelineModel::RunningSessionIdRole).toString(), QString("s-run"));
        QVERIFY(qAbs(backend.fulfilmentRatio().toDouble() - 75.0 / 195.0) < 0.001);
        QCOMPARE(backend.doneItems(), 2);

        // Another surface stops it: the change notification refreshes status.
        state->status = TrackingFixture::status(fixture("status_running"), false);
        state->day = TrackingFixture::day(fixture("day"), false);
        QSignalSpy running(&backend, &Backend::runningChanged);
        daemon.notifyChanged(150, {"sessions"});
        QTRY_VERIFY_WITH_TIMEOUT(backend.running().isEmpty(), 3000);
        QVERIFY(backend.runningTitle().isEmpty());
        QVERIFY(running.size() >= 1);
    }
    void statsFilterAndRangeReachTheDaemon() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        daemon.setHandler(trackingHandler(trackingState()));
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy stats(&backend, &Backend::statsDataChanged);
        backend.loadStatsFiltered(7, "faith", {});
        QTRY_COMPARE_WITH_TIMEOUT(stats.size(), 1, 3000);
        QJsonObject params = lastRequest(daemon, "stats.get").value("params").toObject();
        QCOMPARE(params.value("days").toInt(), 7);
        QCOMPARE(params.value("tag_id").toString(), QString("faith"));
        QVERIFY(!params.contains("project_id"));
        QCOMPARE(backend.statsData().value("filter").toMap().value("tag_id").toString(), QString("faith"));
        QCOMPARE(backend.statsData().value("days").toList().size(), 7);
        backend.loadStatsFiltered(90, {}, "study");
        QTRY_COMPARE_WITH_TIMEOUT(stats.size(), 2, 3000);
        params = lastRequest(daemon, "stats.get").value("params").toObject();
        QCOMPARE(params.value("days").toInt(), 90);
        QCOMPARE(params.value("project_id").toString(), QString("study"));
        QVERIFY(!params.contains("tag_id"));
        // An unfiltered read after a filtered one is not coalesced into it.
        backend.loadStatsFiltered(90, {}, {});
        QTRY_COMPARE_WITH_TIMEOUT(stats.size(), 3, 3000);
        QVERIFY(!lastRequest(daemon, "stats.get").value("params").toObject().contains("project_id"));
        QCOMPARE(backend.statsData().value("days").toList().size(), 90);
    }
    void bumpNoteAndAsksReachTheDaemon() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        daemon.setHandler(trackingHandler(trackingState()));
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy spy(&backend, &Backend::requestFinished);
        QSignalSpy noted(&backend, &Backend::runningNoteFinished);
        backend.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 4, 3000);
        backend.bumpDistractions("s-run", 1);
        QTRY_VERIFY_WITH_TIMEOUT(!lastRequest(daemon, "session.bump").isEmpty(), 3000);
        QJsonObject params = lastRequest(daemon, "session.bump").value("params").toObject();
        QCOMPARE(params.value("id").toString(), QString("s-run"));
        QCOMPARE(params.value("metric_id").toString(), QString("distractions"));
        QCOMPARE(params.value("delta").toInt(), 1);
        backend.bumpDistractions("s-run", -1);
        QTRY_COMPARE_WITH_TIMEOUT(lastRequest(daemon, "session.bump").value("params").toObject().value("delta").toInt(), -1, 3000);
        backend.addRunningNote("s-run", "  Back on track  ");
        QTRY_COMPARE_WITH_TIMEOUT(noted.size(), 1, 3000);
        params = lastRequest(daemon, "session.note").value("params").toObject();
        QCOMPARE(params.value("id").toString(), QString("s-run"));
        QCOMPARE(params.value("text").toString(), QString("Back on track"));
        // Asks: a customised list is sent, "Reset to project" sends null, untouched sends nothing.
        backend.saveItem(QVariantMap{{"id", "quran"}, {"title", "Read Quran"}, {"block", "fajr"}, {"repeat", "daily"},
                                     {"project_id", "quran"}, {"metric_ids", QVariantList{"focus", "pages"}}});
        QTRY_VERIFY_WITH_TIMEOUT(!lastRequest(daemon, "item.edit").isEmpty(), 3000);
        QJsonArray ids = lastRequest(daemon, "item.edit").value("params").toObject().value("metric_ids").toArray();
        QCOMPARE(ids.size(), 2);
        QCOMPARE(ids.at(1).toString(), QString("pages"));
        backend.saveItem(QVariantMap{{"id", "quran"}, {"title", "Read Quran"}, {"block", "fajr"}, {"repeat", "daily"},
                                     {"project_id", "quran"}, {"metric_ids", QVariant()}});
        QTRY_VERIFY_WITH_TIMEOUT(lastRequest(daemon, "item.edit").value("params").toObject().value("metric_ids").isNull(), 3000);
        QVERIFY(lastRequest(daemon, "item.edit").value("params").toObject().contains("metric_ids"));
        backend.saveItem(QVariantMap{{"id", "quran"}, {"title", "Read Quran II"}, {"block", "fajr"}, {"repeat", "daily"}});
        QTRY_COMPARE_WITH_TIMEOUT(lastRequest(daemon, "item.edit").value("params").toObject().value("title").toString(), QString("Read Quran II"), 3000);
        QVERIFY(!lastRequest(daemon, "item.edit").value("params").toObject().contains("metric_ids"));
    }
    void statsTaskAndCustomRangeFiltersReachTheDaemon() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        daemon.setHandler(trackingHandler(trackingState()));
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy stats(&backend, &Backend::statsDataChanged);
        QSignalSpy items(&backend, &Backend::trackingItemsChanged);
        backend.loadTrackingItems();
        QTRY_COMPARE_WITH_TIMEOUT(items.size(), 1, 3000);
        QVERIFY(backend.trackingItems().size() >= 5);
        backend.loadStatsFiltered(28, "learning", "study", "deep");
        QTRY_COMPARE_WITH_TIMEOUT(stats.size(), 1, 3000);
        QJsonObject params = lastRequest(daemon, "stats.get").value("params").toObject();
        QCOMPARE(params.value("tag_id").toString(), QString("learning"));
        QCOMPARE(params.value("project_id").toString(), QString("study"));
        QCOMPARE(params.value("item_id").toString(), QString("deep"));
        backend.loadStatsRange("2031-04-01", "2031-05-14", {}, {}, "deep");
        QTRY_COMPARE_WITH_TIMEOUT(stats.size(), 2, 3000);
        params = lastRequest(daemon, "stats.get").value("params").toObject();
        QCOMPARE(params.value("from").toString(), QString("2031-04-01"));
        QCOMPARE(params.value("to").toString(), QString("2031-05-14"));
        QVERIFY(!params.contains("days"));
        QCOMPARE(params.value("item_id").toString(), QString("deep"));
        QCOMPARE(backend.statsData().value("days").toList().size(), 44);
    }
    void statsTableSessionsListAndMetricFiltersReachTheDaemon() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        daemon.setHandler(trackingHandler(trackingState()));
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy table(&backend, &Backend::statsTableChanged);
        QSignalSpy list(&backend, &Backend::sessionsListChanged);
        QSignalSpy stats(&backend, &Backend::statsDataChanged);
        const QVariantMap focus{{"focus", QVariantMap{{"min", 4}}}, {"distractions", QVariantMap{{"max", 2}}}};
        backend.loadStatsTable(QVariantMap{{"days", 28}, {"project_id", "quran"}, {"minutes_min", 10}, {"values", focus},
                                           {"bogus", 1}}, "project");
        QTRY_COMPARE_WITH_TIMEOUT(table.size(), 1, 3000);
        QJsonObject params = lastRequest(daemon, "stats.table").value("params").toObject();
        QCOMPARE(params.value("group").toString(), QString("project"));
        QCOMPARE(params.value("days").toInt(), 28);
        QCOMPARE(params.value("project_id").toString(), QString("quran"));
        QCOMPARE(params.value("minutes_min").toInt(), 10);
        QVERIFY(!params.contains("minutes_max"));
        QVERIFY(!params.contains("bogus"));
        QCOMPARE(params.value("values").toObject().value("focus").toObject(), (QJsonObject{{"min", 4}}));
        QCOMPARE(params.value("values").toObject().value("distractions").toObject(), (QJsonObject{{"max", 2}}));
        QCOMPARE(backend.statsTable().value("rows").toList().size(), 4);
        QCOMPARE(backend.statsTable().value("rows").toList().last().toMap().value("key").toString(), QString("none"));
        // An unknown group falls back to week; a custom range is sent as from/to.
        backend.loadStatsTable(QVariantMap{{"from", "2031-04-01"}, {"to", "2031-05-14"}, {"days", 7}}, "nope");
        QTRY_COMPARE_WITH_TIMEOUT(table.size(), 2, 3000);
        params = lastRequest(daemon, "stats.table").value("params").toObject();
        QCOMPARE(params.value("group").toString(), QString("week"));
        QCOMPARE(params.value("from").toString(), QString("2031-04-01"));
        QVERIFY(!params.contains("days"));

        // stats.get carries the session filters too.
        backend.loadStatsQuery(QVariantMap{{"days", 90}, {"minutes_max", 45}, {"values", focus}});
        QTRY_COMPARE_WITH_TIMEOUT(stats.size(), 1, 3000);
        params = lastRequest(daemon, "stats.get").value("params").toObject();
        QCOMPARE(params.value("days").toInt(), 90);
        QCOMPARE(params.value("minutes_max").toInt(), 45);
        QCOMPARE(params.value("values").toObject().value("focus").toObject(), (QJsonObject{{"min", 4}}));
        backend.loadStatsQuery(QVariantMap{{"days", 7}, {"tag_id", "faith"}});
        QTRY_COMPARE_WITH_TIMEOUT(stats.size(), 2, 3000);
        params = lastRequest(daemon, "stats.get").value("params").toObject();
        QVERIFY(!params.contains("values"));
        QCOMPARE(params.value("tag_id").toString(), QString("faith"));

        // sessions.list: with_note, a trimmed q, a limit, then the cursor.
        backend.loadSessions(QVariantMap{{"days", 90}, {"with_note", false}});
        QTRY_COMPARE_WITH_TIMEOUT(list.size(), 1, 3000);
        params = lastRequest(daemon, "sessions.list").value("params").toObject();
        QCOMPARE(params.value("with_note"), QJsonValue(false));
        QCOMPARE(params.value("limit").toInt(), 100);
        QVERIFY(!params.contains("before"));
        QCOMPARE(backend.sessionsList().size(), 5);
        QVERIFY(backend.sessionsHasMore());
        backend.loadMoreSessions();
        QTRY_COMPARE_WITH_TIMEOUT(list.size(), 2, 3000);
        params = lastRequest(daemon, "sessions.list").value("params").toObject();
        QCOMPARE(params.value("before").toString(), QString("2031-05-11|2031-05-11T09:00:00Z|l-5"));
        QCOMPARE(params.value("with_note"), QJsonValue(false));
        QCOMPARE(backend.sessionsList().size(), 6);
        QVERIFY(!backend.sessionsHasMore());
        backend.loadSessions(QVariantMap{{"days", 28}, {"with_note", true}, {"q", "  dative "}, {"values", focus}});
        QTRY_COMPARE_WITH_TIMEOUT(list.size(), 3, 3000);
        params = lastRequest(daemon, "sessions.list").value("params").toObject();
        QCOMPARE(params.value("q").toString(), QString("dative"));
        QCOMPARE(params.value("with_note"), QJsonValue(true));
        QCOMPARE(params.value("values").toObject().value("distractions").toObject(), (QJsonObject{{"max", 2}}));
        QCOMPARE(backend.sessionsList().size(), 3);
        QVERIFY(backend.error().isEmpty());
    }
    void canonicalTableAndListFixturesReachTheModels() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        daemon.setHandler([](const QJsonObject &request, QLocalSocket *socket, FakeDaemon *server) {
            const QString method = request.value("method").toString();
            const QJsonObject params = request.value("params").toObject();
            if (method == "stats.table" && params.value("project_id") == "broken")
                server->respondError(socket, request, -32602, "invalid_params", "unknown project");
            else if (method == "stats.table" && params.contains("values"))
                server->respond(socket, request, fixture("stats_table_metric_filter"));
            else if (method == "stats.table" && params.value("group") == "tag")
                server->respond(socket, request, fixture("stats_table_tag_items"));
            else if (method == "stats.table")
                server->respond(socket, request, fixture(params.value("group").toString() == "project" ? "stats_table_project" : "stats_table_week"));
            else if (method == "stats.get")
                server->respond(socket, request, fixture("stats_metric_filter"));
            else if (method == "sessions.list")
                server->respond(socket, request, fixture(params.contains("values") ? "sessions_list_filtered" : "sessions_list"));
            else server->respond(socket, request, QJsonObject{});
        });
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy table(&backend, &Backend::statsTableChanged);
        QSignalSpy list(&backend, &Backend::sessionsListChanged);
        backend.loadStatsTable(QVariantMap{{"days", 14}}, "week");
        QTRY_COMPARE_WITH_TIMEOUT(table.size(), 1, 3000);
        QVariantList rows = backend.statsTable().value("rows").toList();
        QCOMPARE(rows.size(), 3);
        QCOMPARE(rows.first().toMap().value("sessions").toInt(), 0);
        QCOMPARE(backend.statsTable().value("columns").toList().last().toMap().value("derived_from").toString(), QString("distractions"));
        backend.loadStatsTable(QVariantMap{{"days", 14}}, "project");
        QTRY_COMPARE_WITH_TIMEOUT(table.size(), 2, 3000);
        rows = backend.statsTable().value("rows").toList();
        QCOMPARE(rows.last().toMap().value("key").toString(), QString("none"));
        QCOMPARE(rows.last().toMap().value("label").toString(), QString("No project"));
        QVERIFY(rows.last().toMap().value("color").isNull());
        backend.loadSessions(QVariantMap{{"days", 14}});
        QTRY_COMPARE_WITH_TIMEOUT(list.size(), 1, 3000);
        QCOMPARE(backend.sessionsList().size(), 3);
        QVERIFY(!backend.sessionsHasMore());
        QCOMPARE(backend.sessionsList().first().toMap().value("notes").toList().size(), 1);
        backend.loadSessions(QVariantMap{{"days", 14}, {"values", QVariantMap{{"focus", QVariantMap{{"min", 4}}}}}});
        QTRY_COMPARE_WITH_TIMEOUT(list.size(), 2, 3000);
        QCOMPARE(backend.sessionsList().size(), 1);
        // The metric-filter fixtures decode, and echo the filter that was sent.
        const QVariantMap focusRange{{"focus", QVariantMap{{"min", 3}, {"max", 4}}}};
        backend.loadStatsTable(QVariantMap{{"days", 14}, {"tag_id", "t"}, {"minutes_min", 10}, {"values", focusRange}}, "project");
        QTRY_COMPARE_WITH_TIMEOUT(table.size(), 3, 3000);
        QCOMPARE(backend.statsTable().value("filter").toMap().value("minutes_min").toInt(), 10);
        QCOMPARE(backend.statsTable().value("rows").toList().size(), 1);
        // A failed query clears the previous rows instead of showing them
        // under the new filter.
        backend.loadStatsTable(QVariantMap{{"days", 14}, {"project_id", "broken"}}, "project");
        QTRY_COMPARE_WITH_TIMEOUT(table.size(), 4, 3000);
        QVERIFY(backend.statsTable().isEmpty());
        QCOMPARE(backend.error(), QString("unknown project"));
        backend.dismissError();
        QSignalSpy stats(&backend, &Backend::statsDataChanged);
        backend.loadStatsQuery(QVariantMap{{"days", 14}, {"minutes_min", 10}, {"values", QVariantMap{{"focus", QVariantMap{{"min", 3}}}}}});
        QTRY_COMPARE_WITH_TIMEOUT(stats.size(), 1, 3000);
        QCOMPARE(backend.statsData().value("filter").toMap().value("values").toMap().value("focus").toMap().value("min").toDouble(), 3.0);
        QCOMPARE(backend.statsData().value("totals").toMap().value("sessions").toInt(), 2);
        backend.loadStatsTable(QVariantMap{{"days", 1}}, "tag");
        QTRY_COMPARE_WITH_TIMEOUT(table.size(), 5, 3000);
        rows = backend.statsTable().value("rows").toList();
        QCOMPARE(rows.size(), 2);
        QCOMPARE(rows.first().toMap().value("minutes").toInt(), 80);
        QCOMPARE(rows.last().toMap().value("key").toString(), QString("none"));
        QCOMPARE(rows.last().toMap().value("sessions").toInt(), 1);
        QCOMPARE(backend.statsTable().value("totals").toMap().value("minutes").toInt(), 100);
        QVERIFY(backend.error().isEmpty());
    }
    void epicsFollowCanonicalFixtures() {
        const QString rpc = QStringLiteral(TEST_SOURCE_DIR "/../../crates/salah-cli/tests/fixtures/rpc/");
        for (const char *name : {"epic_add", "epic_edit_finish", "session_log_epic", "item_add_quick_tomorrow"})
            QVERIFY2(Dto::decode<Dto::MutationResult>(fixture(name)), name);
        QVERIFY(Dto::decode<Dto::ParseDraft>(fixture("item_parse_tomorrow")));
        QCOMPARE(fixture("item_parse_tomorrow").value("start_day").toString(), QString("2031-05-15"));
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        const QJsonValue epicsList = readJson(rpc + "epics_list.json").object().value("result");
        const auto base = trackingHandler(trackingState());
        daemon.setHandler([epicsList, base](const QJsonObject &request, QLocalSocket *socket, FakeDaemon *server) {
            const QString method = request.value("method").toString();
            if (method == "epics.list") server->respond(socket, request, epicsList);
            else if (method == "epic.add") server->respond(socket, request, fixture("epic_add"));
            else if (method == "epic.edit") server->respond(socket, request, fixture("epic_edit_finish"));
            else if (method == "epic.remove") server->respond(socket, request, fixture("epic_edit_finish"));
            else if (method == "session.log") server->respond(socket, request, fixture("session_log_epic"));
            else if (method == "stats.table") server->respond(socket, request, fixture("stats_table_epic"));
            else base(request, socket, server);
        });
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy settings(&backend, &Backend::settingsChanged);
        backend.loadSettings();
        QTRY_VERIFY_WITH_TIMEOUT(!backend.epics().isEmpty(), 3000);
        const QVariantMap read = backend.epics().first().toMap();
        QCOMPARE(read.value("name").toString(), QString("Read"));
        QCOMPARE(read.value("total_minutes").toInt(), 30);
        QCOMPARE(read.value("status").toString(), QString("active"));

        QSignalSpy saved(&backend, &Backend::entitySaved);
        backend.saveEntity("epic", QVariantMap{{"project_id", "p"}, {"name", "Read"}, {"note", "Spring"}});
        QTRY_COMPARE_WITH_TIMEOUT(saved.size(), 1, 3000);
        QCOMPARE(saved.first().at(0).toString(), QString("epic"));
        QCOMPARE(saved.first().at(1).toString(), QString("<id:1>"));
        QJsonObject params = lastRequest(daemon, "epic.add").value("params").toObject();
        QCOMPARE(params.value("project_id").toString(), QString("p"));
        QCOMPARE(params.value("note").toString(), QString("Spring"));
        QVERIFY(!params.value("op_id").toString().isEmpty());
        backend.saveEntity("epic", QVariantMap{{"id", "<id:1>"}, {"status", "finished"}});
        QTRY_COMPARE_WITH_TIMEOUT(saved.size(), 2, 3000);
        params = lastRequest(daemon, "epic.edit").value("params").toObject();
        QCOMPARE(params.value("status").toString(), QString("finished"));
        QVERIFY(!params.contains("name"));
        backend.removeEpic("<id:1>");
        QTRY_COMPARE_WITH_TIMEOUT(saved.size(), 3, 3000);
        QCOMPARE(lastRequest(daemon, "epic.remove").value("params").toObject().value("id").toString(), QString("<id:1>"));
        QVERIFY2(backend.error().isEmpty(), qPrintable(backend.error()));

        // Work logged against the epic alone: epic_id, no project_id, no item_id.
        QSignalSpy writes(&backend, &Backend::sessionWriteFinished);
        backend.logSession({}, "p", 30, QVariantMap{{"focus", 4}}, {}, "<id:1>");
        QTRY_COMPARE_WITH_TIMEOUT(writes.size(), 1, 3000);
        params = lastRequest(daemon, "session.log").value("params").toObject();
        QCOMPARE(params.value("epic_id").toString(), QString("<id:1>"));
        QVERIFY(!params.contains("project_id"));
        QVERIFY(!params.contains("item_id"));
        QCOMPARE(params.value("minutes").toInt(), 30);

        // The epic table keeps its unassigned row, drawn without a colour.
        QSignalSpy table(&backend, &Backend::statsTableChanged);
        backend.loadStatsTable(QVariantMap{{"days", 14}, {"epic_id", "none"}}, "epic");
        QTRY_COMPARE_WITH_TIMEOUT(table.size(), 1, 3000);
        params = lastRequest(daemon, "stats.table").value("params").toObject();
        QCOMPARE(params.value("group").toString(), QString("epic"));
        QCOMPARE(params.value("epic_id").toString(), QString("none"));
        const QVariantList rows = backend.statsTable().value("rows").toList();
        QCOMPARE(rows.last().toMap().value("label").toString(), QString("No epic"));
        QVERIFY(rows.last().toMap().value("color").isNull());
    }
    void epicFiltersAndEditorFieldsReachTheDaemon() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        daemon.setHandler(trackingHandler(trackingState()));
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy stats(&backend, &Backend::statsDataChanged);
        backend.loadStatsQuery(QVariantMap{{"days", 28}, {"epic_id", "e-juz"}});
        QTRY_COMPARE_WITH_TIMEOUT(stats.size(), 1, 3000);
        QCOMPARE(lastRequest(daemon, "stats.get").value("params").toObject().value("epic_id").toString(), QString("e-juz"));
        QCOMPARE(backend.statsData().value("epics").toList().size(), 6);
        QSignalSpy table(&backend, &Backend::statsTableChanged);
        backend.loadStatsTable(QVariantMap{{"days", 28}, {"epic_id", "none"}}, "epic");
        QTRY_COMPARE_WITH_TIMEOUT(table.size(), 1, 3000);
        QCOMPARE(lastRequest(daemon, "stats.table").value("params").toObject().value("group").toString(), QString("epic"));
        QSignalSpy list(&backend, &Backend::sessionsListChanged);
        backend.loadSessions(QVariantMap{{"days", 28}, {"epic_id", "e-juz"}});
        QTRY_COMPARE_WITH_TIMEOUT(list.size(), 1, 3000);
        QCOMPARE(lastRequest(daemon, "sessions.list").value("params").toObject().value("epic_id").toString(), QString("e-juz"));
        QSignalSpy trends(&backend, &Backend::trendsChanged);
        backend.loadTrends(QVariantMap{{"days", 28}, {"epic_id", "e-juz"}}, {"minutes"}, "day", "epic");
        QTRY_COMPARE_WITH_TIMEOUT(trends.size(), 1, 3000);
        QJsonObject params = lastRequest(daemon, "stats.series").value("params").toObject();
        QCOMPARE(params.value("split").toString(), QString("epic"));
        QCOMPARE(params.value("epic_id").toString(), QString("e-juz"));
        QCOMPARE(backend.trends().value("split").toString(), QString("epic"));

        // The item editor sends epic_id only when it sets or clears one.
        backend.saveItem(QVariantMap{{"id", "quran"}, {"title", "Read Quran"}, {"block", "fajr"}, {"project_id", "quran"},
                                     {"epic_id", "e-tajweed"}});
        QTRY_VERIFY_WITH_TIMEOUT(!lastRequest(daemon, "item.edit").isEmpty(), 3000);
        QCOMPARE(lastRequest(daemon, "item.edit").value("params").toObject().value("epic_id").toString(), QString("e-tajweed"));
        backend.saveItem(QVariantMap{{"id", "quran"}, {"title", "Read Quran"}, {"block", "fajr"}, {"project_id", "quran"},
                                     {"epic_id", ""}});
        QTRY_VERIFY_WITH_TIMEOUT(lastRequest(daemon, "item.edit").value("params").toObject().value("epic_id").isNull()
                                 && lastRequest(daemon, "item.edit").value("params").toObject().contains("epic_id"), 3000);
        const int edits = requestCount(daemon, "item.edit");
        backend.saveItem(QVariantMap{{"id", "quran"}, {"title", "Read Quran II"}, {"block", "fajr"}, {"project_id", "quran"}});
        QTRY_COMPARE_WITH_TIMEOUT(requestCount(daemon, "item.edit"), edits + 1, 3000);
        QVERIFY(!lastRequest(daemon, "item.edit").value("params").toObject().contains("epic_id"));
        backend.saveItem(QVariantMap{{"title", "Hifz"}, {"block", "fajr"}, {"project_id", "quran"}, {"epic_id", "e-juz"}});
        QTRY_VERIFY_WITH_TIMEOUT(!lastRequest(daemon, "item.add").isEmpty(), 3000);
        QCOMPARE(lastRequest(daemon, "item.add").value("params").toObject().value("epic_id").toString(), QString("e-juz"));

        // The Epic page reads minutes per day and the epic's own sessions.
        QSignalSpy page(&backend, &Backend::epicPageChanged);
        backend.loadEpicPage("e-juz");
        QTRY_VERIFY_WITH_TIMEOUT(!backend.epicSessions().isEmpty() && !backend.epicSeries().isEmpty(), 3000);
        params = lastRequest(daemon, "stats.series").value("params").toObject();
        QCOMPARE(params.value("epic_id").toString(), QString("e-juz"));
        QCOMPARE(params.value("measures").toArray(), QJsonArray{"minutes"});
        QCOMPARE(lastRequest(daemon, "sessions.list").value("params").toObject().value("epic_id").toString(), QString("e-juz"));
        QCOMPARE(backend.epicSessions().size(), 1);
        QCOMPARE(backend.epicSessions().first().toMap().value("epic_id").toString(), QString("e-juz"));
        QVERIFY(backend.error().isEmpty());
    }
    void pausePanelCapsWorkedAtMaxMinutes() {
        QTemporaryDir dir;
        Backend backend(nullptr, fake(dir), 3000, daemon_path(dir));
        Theme theme(dir.path());
        QQmlEngine engine;
        engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl::fromLocalFile(QStringLiteral(TEST_SOURCE_DIR "/../src/qml/Main.qml")));
        QScopedPointer<QObject> root(component.create());
        QVERIFY2(root, qPrintable(component.errorString()));
        QObject *prompt = findByName(root.data(), "sessionPrompt");
        QObject *worked = findByName(root.data(), "sessionMinutesField");
        QVERIFY(prompt && worked);
        QVariantMap options{{"mode", "pause"}, {"sessionId", "s-run"}, {"itemId", "deep"}, {"title", "Deep work"},
                            {"minutes", 600}, {"maxMinutes", 120}, {"stale", true}};
        QVERIFY(QMetaObject::invokeMethod(prompt, "show", Q_ARG(QVariant, QVariant(options))));
        QTRY_VERIFY(prompt->property("visible").toBool());
        QCOMPARE(worked->property("text").toString(), QString("120"));
        auto *content = prompt->property("contentItem").value<QQuickItem *>();
        QVERIFY(content);
        QQuickItem *staleNote = nullptr;
        QQuickItem *chip30 = nullptr;
        for (QQuickItem *item : quickDescendants(content)) {
            if (item->objectName() == "sessionStaleNote") staleNote = item;
            if (item->objectName() == "subtractWorked30") chip30 = item;
        }
        QVERIFY(staleNote && chip30);
        QVERIFY(staleNote->isVisible());
        QVERIFY(staleNote->property("text").toString().contains("120"));
        // Above the cap is not acceptable input; a chip starts from the cap.
        worked->setProperty("text", "121");
        QVERIFY(!worked->property("acceptableInput").toBool());
        QVERIFY(QMetaObject::invokeMethod(chip30, "clicked"));
        QCOMPARE(worked->property("text").toString(), QString("90"));
        worked->setProperty("text", "20");
        QVERIFY(QMetaObject::invokeMethod(chip30, "clicked"));
        QCOMPARE(worked->property("text").toString(), QString("1"));
        // A fresh timer: no stale line, Worked is the elapsed time under the cap.
        options["minutes"] = 47;
        options["maxMinutes"] = 47;
        options["stale"] = false;
        QVERIFY(QMetaObject::invokeMethod(prompt, "show", Q_ARG(QVariant, QVariant(options))));
        QCOMPARE(worked->property("text").toString(), QString("47"));
        QVERIFY(!staleNote->isVisible());
    }
    void staleRunningCardFreezesAtSleepClamp() {
        QTemporaryDir dir;
        Backend backend(nullptr, fake(dir), 3000, daemon_path(dir));
        Theme theme(dir.path());
        QQmlEngine engine;
        engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl(QStringLiteral("qrc:/qml/Main.qml")));
        QScopedPointer<QObject> root(component.create());
        QVERIFY2(root, qPrintable(component.errorString()));
        auto *window = qobject_cast<QQuickWindow *>(root.data());
        QVERIFY(window);
        window->resize(1440, 900);
        auto *card = findByName(root.data(), "runningCard");
        QVERIFY(card);
        auto *elapsed = findByName(card, "runningElapsed");
        auto *note = findByName(card, "runningStaleNote");
        QVERIFY(elapsed && note);
        auto status = TrackingFixture::status(fixture("status_running"));
        auto reloadStatus = [&] {
            write(dir.filePath("status.json"), QJsonDocument(status).toJson(QJsonDocument::Compact));
            backend.reload();
            waitForBackendIdle(backend);
        };
        reloadStatus();
        QVERIFY(card->property("visible").toBool());
        QVERIFY(!note->property("visible").toBool());
        const auto started = QDateTime::fromString(backend.running().value("started_at").toString(), Qt::ISODate);
        QVERIFY(started.isValid());
        root->setProperty("timerNow", started.addSecs(47 * 60 + 5).toMSecsSinceEpoch());
        QCOMPARE(elapsed->property("text").toString(), QString("47:05"));
        root->setProperty("timerNow", started.addSecs(47 * 60 + 6).toMSecsSinceEpoch());
        QCOMPARE(elapsed->property("text").toString(), QString("47:06"));

        status["running"] = TrackingFixture::staleRunningSession();
        reloadStatus();
        QVERIFY(note->property("visible").toBool());
        QCOMPARE(note->property("text").toString(), QString("Ran past Sleep: stop it to save up to 120 min"));
        QCOMPARE(elapsed->property("text").toString(), QString("120:00"));
        root->setProperty("timerNow", QDateTime::currentMSecsSinceEpoch() + 60 * 60 * 1000);
        QCOMPARE(elapsed->property("text").toString(), QString("120:00"));
        shotWindow(window, "running-card-stale.png");
        for (const char *button : {"distractionPlus", "distractionMinus", "runningAddNoteButton"}) {
            auto *control = findByName(card, button);
            QVERIFY(control && !control->property("enabled").toBool());
        }
        // Pause stays available: it is the way to save a stale session.
        auto *stalePause = findByName(card, "runningStopButton");
        QVERIFY(stalePause && stalePause->property("enabled").toBool());
        QQmlComponent rowComponent(&engine, QUrl("qrc:/qml/today/ItemCard.qml"));
        QScopedPointer<QObject> row(rowComponent.createWithInitialProperties({
            {"win", QVariant::fromValue(root.data())}, {"timerRunning", true},
            {"runningSessionId", "s-run"}, {"itemId", "deep"}}));
        QVERIFY2(row, qPrintable(rowComponent.errorString()));
        auto *pause = findByName(row.data(), "timerButton");
        QVERIFY(pause);
        QVERIFY(pause->property("text").toString().contains("120:00"));
        row->setProperty("runningSessionId", "previous-session");
        QVERIFY(QMetaObject::invokeMethod(pause, "clicked"));
        QVERIFY(!root->property("sessionPromptVisible").toBool());
        waitForBackendIdle(backend);
        row->setProperty("runningSessionId", "s-run");
        QVERIFY(QMetaObject::invokeMethod(pause, "clicked"));
        QVERIFY(root->property("sessionPromptVisible").toBool());
        auto *prompt = findByName(root.data(), "sessionPrompt");
        auto *worked = findByName(root.data(), "sessionMinutesField");
        QVERIFY(prompt && worked);
        worked->setProperty("text", "90");
        auto updated = status.value("running").toObject();
        updated["max_minutes"] = 45;
        status["running"] = updated;
        reloadStatus();
        QCOMPARE(prompt->property("workedCap").toInt(), 45);
        QVERIFY(!worked->property("acceptableInput").toBool());
        QVERIFY(QMetaObject::invokeMethod(prompt, "save", Q_ARG(QVariant, false), Q_ARG(QVariant, false)));
        QVERIFY(!prompt->property("waiting").toBool());
        QVERIFY(prompt->property("serverError").toString().contains("45"));
        QVERIFY(offlineRequests(dir).isEmpty());
    }
    void filterBarTableAndTabsFollowTheUser() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        daemon.setHandler(trackingHandler(trackingState()));
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        Theme theme(dir.path());
        QQmlEngine engine; engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl::fromLocalFile(QStringLiteral(TEST_SOURCE_DIR "/../src/qml/Main.qml")));
        QScopedPointer<QObject> root(component.create()); QVERIFY2(root, qPrintable(component.errorString()));
        backend.reload();
        waitForBackendIdle(backend);
        QTRY_COMPARE_WITH_TIMEOUT(backend.metrics().size(), 7, 3000);
        root->setProperty("page", 2);
        QObject *stats = findByName(root.data(), "statsScroll"); QVERIFY(stats);
        QObject *bar = stats->property("filterBar").value<QObject *>(); QVERIFY(bar);
        QCOMPARE(stats->property("tab").toString(), QString("charts"));
        QVERIFY(QMetaObject::invokeMethod(root.data(), "loadStatistics"));
        waitForBackendIdle(backend);
        // Two metric ranges and a duration: chips, and the same filter on every tab.
        QVERIFY(QMetaObject::invokeMethod(bar, "setMetricRange", Q_ARG(QVariant, QVariant("focus")), Q_ARG(QVariant, QVariant(4)), Q_ARG(QVariant, QVariant(""))));
        QVERIFY(QMetaObject::invokeMethod(bar, "setMetricRange", Q_ARG(QVariant, QVariant("distractions")), Q_ARG(QVariant, QVariant("")), Q_ARG(QVariant, QVariant(2))));
        QVERIFY(QMetaObject::invokeMethod(bar, "setMetricRange", Q_ARG(QVariant, QVariant("minutes")), Q_ARG(QVariant, QVariant(60)), Q_ARG(QVariant, QVariant(10))));
        waitForBackendIdle(backend);
        QVariantList chips = bar->property("activeChips").toList();
        QJsonObject seriesParams = lastRequest(daemon, "stats.series").value("params").toObject();
        QCOMPARE(seriesParams.take("measures").toArray(), QJsonArray{"minutes"});
        QCOMPARE(seriesParams.take("interval").toString(), QString("day"));
        QCOMPARE(seriesParams, lastRequest(daemon, "stats.get").value("params").toObject());
        QCOMPARE(chips.size(), 3);
        QCOMPARE(chips.at(0).toMap().value("text").toString(), QString("Duration 10–60 min"));
        QCOMPARE(chips.at(1).toMap().value("text").toString(), QString::fromUtf8("Focus ≥ 4"));
        QCOMPARE(chips.at(2).toMap().value("text").toString(), QString::fromUtf8("Distractions ≤ 2"));
        QJsonObject params = lastRequest(daemon, "stats.get").value("params").toObject();
        QCOMPARE(params.value("minutes_min").toInt(), 10);
        QCOMPARE(params.value("minutes_max").toInt(), 60);
        QCOMPARE(params.value("values").toObject().value("focus").toObject(), (QJsonObject{{"min", 4}}));
        QVERIFY(QMetaObject::invokeMethod(bar, "removeChip", Q_ARG(QVariant, QVariant("minutes"))));
        waitForBackendIdle(backend);
        QVERIFY(!lastRequest(daemon, "stats.get").value("params").toObject().contains("minutes_min"));

        QVERIFY(QMetaObject::invokeMethod(stats, "setTab", Q_ARG(QVariant, QVariant("table"))));
        waitForBackendIdle(backend);
        params = lastRequest(daemon, "stats.table").value("params").toObject();
        QCOMPARE(params.value("group").toString(), QString("week"));
        QCOMPARE(params.value("values").toObject().value("distractions").toObject(), (QJsonObject{{"max", 2}}));
        QObject *table = findByName(root.data(), "generalTable"); QVERIFY(table);
        QCOMPARE(table->property("rows").toList().size(), 5);
        QVERIFY(QMetaObject::invokeMethod(table, "setGroup", Q_ARG(QVariant, QVariant("project"))));
        waitForBackendIdle(backend);
        QCOMPARE(lastRequest(daemon, "stats.table").value("params").toObject().value("group").toString(), QString("project"));
        // Sort by focus: descending, ascending (nulls last both ways), then server order.
        QVERIFY(QMetaObject::invokeMethod(table, "sortBy", Q_ARG(QVariant, QVariant("focus"))));
        QVariantList rows = table->property("rows").toList();
        QCOMPARE(rows.first().toMap().value("key").toString(), QString("quran"));
        QCOMPARE(rows.last().toMap().value("key").toString(), QString("none"));
        QVERIFY(QMetaObject::invokeMethod(table, "sortBy", Q_ARG(QVariant, QVariant("focus"))));
        rows = table->property("rows").toList();
        QCOMPARE(rows.first().toMap().value("key").toString(), QString("fitness"));
        QCOMPARE(rows.last().toMap().value("key").toString(), QString("none"));
        QVERIFY(QMetaObject::invokeMethod(table, "sortBy", Q_ARG(QVariant, QVariant("focus"))));
        QCOMPARE(table->property("sortKey").toString(), QString());
        // Hidden columns are remembered.
        QVERIFY(QMetaObject::invokeMethod(table, "toggleColumn", Q_ARG(QVariant, QVariant("quality"))));
        QCOMPARE(backend.uiSetting("statistics/tableHidden").toString(), QString("quality"));
        QCOMPARE(table->property("shown").toList().size(), 6);

        QVERIFY(QMetaObject::invokeMethod(stats, "setTab", Q_ARG(QVariant, QVariant("notes"))));
        waitForBackendIdle(backend);
        params = lastRequest(daemon, "sessions.list").value("params").toObject();
        QCOMPARE(params.value("with_note"), QJsonValue(true));
        QCOMPARE(params.value("values").toObject().value("focus").toObject(), (QJsonObject{{"min", 4}}));
        QCOMPARE(backend.uiSetting("statistics/tab").toString(), QString("notes"));
        QCOMPARE(backend.sessionsList().size(), 3);
        // Clicking a note opens the session editor on that session.
        QObject *notes = findByName(root.data(), "notesList"); QVERIFY(notes);
        QVERIFY(QMetaObject::invokeMethod(notes, "openSession", Q_ARG(QVariant, backend.sessionsList().first())));
        QObject *prompt = findByName(root.data(), "sessionPrompt"); QVERIFY(prompt);
        QTRY_VERIFY(prompt->property("visible").toBool());
        QCOMPARE(prompt->property("mode").toString(), QString("edit"));
        QCOMPARE(prompt->property("sessionId").toString(), QString("l-1"));
        root->setProperty("sessionPromptVisible", false);
        QVERIFY(QMetaObject::invokeMethod(bar, "clearMetrics"));
        waitForBackendIdle(backend);
        QVERIFY(!lastRequest(daemon, "sessions.list").value("params").toObject().contains("values"));
        QVERIFY(backend.uiSetting("statistics/tab").isValid());

        // A new page instance comes back on the remembered tab.
        QScopedPointer<QObject> again(component.create()); QVERIFY(again);
        QCOMPARE(findByName(again.data(), "statsScroll")->property("tab").toString(), QString("notes"));
        QVERIFY2(backend.error().isEmpty(), qPrintable(backend.error()));
    }
    void trendsUiPreviews() {
        QTemporaryDir dir;
        fake(dir);
        const QString cli = dir.filePath("reports-cli");
        auto saveReport = [&](const QString &name, const QJsonObject &report) {
            write(dir.filePath(name + ".json"), QJsonDocument(report).toJson(QJsonDocument::Compact));
        };
        saveReport("tasks", TrackingFixture::trendMonth({"focus"}, true));
        saveReport("mixed", TrackingFixture::trendMonth({"focus", "minutes"}, false));
        saveReport("minutes", TrackingFixture::trendMonth({"minutes"}, false));
        // The CLI serves the same series as the socket screenshot test. Log
        // the arguments so range and metric filters are checked in both modes.
        write(cli, "#!/bin/sh\ncd '" + dir.path().toUtf8() + "'\n" + R"SH(
if [ "$1 $2" = "stats series" ]; then
    printf '%s\n' "$*" >> series-args
    case " $* " in
        *" --split item "*) cat tasks.json ;;
        *" --measure focus "*) cat mixed.json ;;
        *) cat minutes.json ;;
    esac
else
    exec ./salah "$@"
fi
)SH");
        QFile::setPermissions(cli, QFile::ReadOwner | QFile::WriteOwner | QFile::ExeOwner);
        Backend backend(nullptr, cli, 3000, daemon_path(dir));
        Theme theme(dir.path());
        QQmlEngine engine;
        engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl(QStringLiteral("qrc:/qml/Main.qml")));
        QScopedPointer<QObject> root(component.create());
        QVERIFY2(root, qPrintable(component.errorString()));
        auto *window = qobject_cast<QQuickWindow *>(root.data());
        QVERIFY(window);
        window->resize(1280, 820);
        root->setProperty("page", 2);
        QVERIFY(QMetaObject::invokeMethod(root.data(), "loadStatistics"));
        waitForBackendIdle(backend);
        backend.dismissError();
        auto *stats = findByName(root.data(), "statsScroll");
        QVERIFY(stats);
        trendsScreens(window, stats);
        waitForBackendIdle(backend);

        QFile args(dir.filePath("series-args"));
        QVERIFY(args.open(QIODevice::ReadOnly));
        QVERIFY(args.readAll().contains("stats series --days 28 --interval day --split item --measure focus\n"));
        auto *chart = findByName(stats, "trendsChart");
        QVERIFY(chart);
        chart->setProperty("report", fixture("stats_series_day").toVariantMap());
        QCOMPARE(chart->property("groups").toList().size(), 3);
        chart->setProperty("report", fixture("stats_series_split_item").toVariantMap());
        QCOMPARE(chart->property("groups").toList().size(), 1);
        auto overflow = fixture("stats_series_split_item");
        auto series = overflow.value("series").toArray();
        while (series.size() < 9) {
            auto line = series.first().toObject();
            line["key"] = QString::number(series.size());
            series.append(line);
        }
        overflow["series"] = series;
        chart->setProperty("report", overflow.toVariantMap());
        QCOMPARE(chart->property("shown").toList().size(), 8);
        auto *ninth = findItemByName(chart, "trendsLine_8");
        QVERIFY(ninth && !ninth->property("enabled").toBool());
        QVERIFY(QMetaObject::invokeMethod(findItemByName(chart, "trendsLine_0"), "clicked"));
        QVERIFY(ninth->property("enabled").toBool());
        QVERIFY(QMetaObject::invokeMethod(ninth, "clicked"));
        QCOMPARE(chart->property("shown").toList().size(), 8);
        const QVariant day31("2031-04-01");
        QVERIFY(QMetaObject::invokeMethod(stats, "setCustom", Q_ARG(QVariant, day31), Q_ARG(QVariant, QVariant("2031-05-01"))));
        QCOMPARE(chart->property("interval").toString(), QString("day"));
        QVERIFY(QMetaObject::invokeMethod(stats, "setCustom", Q_ARG(QVariant, day31), Q_ARG(QVariant, QVariant("2031-05-02"))));
        QCOMPARE(chart->property("interval").toString(), QString("week"));
        QVERIFY(QMetaObject::invokeMethod(findItemByName(chart, "trendsInterval_day"), "clicked"));
        QVERIFY(QMetaObject::invokeMethod(stats, "setFilter", Q_ARG(QVariant, QVariant("project")), Q_ARG(QVariant, QVariant("study"))));
        QCOMPARE(chart->property("interval").toString(), QString("day"));
        auto *bar = stats->property("filterBar").value<QObject *>();
        QVERIFY(bar);
        QVERIFY(QMetaObject::invokeMethod(bar, "setMetricRange", Q_ARG(QVariant, QVariant("minutes")),
                                          Q_ARG(QVariant, QVariant(10)), Q_ARG(QVariant, QVariant(60))));
        QVERIFY(QMetaObject::invokeMethod(bar, "setMetricRange", Q_ARG(QVariant, QVariant("focus")),
                                          Q_ARG(QVariant, QVariant(3.5)), Q_ARG(QVariant, QVariant(""))));
        waitForBackendIdle(backend);
        const auto filtered = args.readAll().trimmed().split('\n').last();
        QCOMPARE(filtered, QByteArray("stats series --from 2031-04-01 --to 2031-05-02 --project study "
                                      "--min-minutes 10 --max-minutes 60 --interval day --measure minutes --min focus=3.5"));
    }

    static QJsonArray backlogRows() {
        const QJsonObject base = readJson(QStringLiteral(TEST_SOURCE_DIR "/../../crates/salah-cli/tests/fixtures/rpc/backlog_list.json")).object().value("result").toArray().first().toObject();
        auto row = [&](const QString &id, const QString &title, const QString &project, const QString &name,
                       const QString &color, const QString &epic, int minutes, int notes, int links,
                       const QJsonArray &tags = {}, const QJsonArray &defaults = {}) {
            QJsonObject r = base;
            r["id"] = id; r["title"] = title; r["epic_id"] = epic.isEmpty() ? QJsonValue(QJsonValue::Null) : QJsonValue(epic);
            r["project_id"] = project.isEmpty() ? QJsonValue(QJsonValue::Null) : QJsonValue(project);
            r["tag_ids"] = tags;
            r["project"] = project.isEmpty() ? QJsonValue(QJsonValue::Null)
                : QJsonValue(QJsonObject{{"id", project}, {"name", name}, {"color", color}, {"tag_ids", defaults}});
            r["planned_minutes"] = minutes > 0 ? QJsonValue(minutes) : QJsonValue(QJsonValue::Null);
            r["resources"] = QJsonObject{{"notes", notes}, {"links", links}, {"reminders", 0}};
            return r;
        };
        return QJsonArray{
            row("b1", "Read chapter 4", "study", "Study", "#4f7a6a", "e-grammar", 40, 1, 2, {"faith"}, {"learning"}),
            row("b2", "Verb tables", "study", "Study", "#4f7a6a", "e-grammar", 25, 0, 0, {"learning"}, {"learning"}),
            row("b3", "Sort the reading list", "study", "Study", "#4f7a6a", "", 0, 0, 1, {}, {"learning"}),
            row("b4", "Review Juz 30", "quran", "Quran", "#7a6a4f", "e-juz", 30, 2, 0, {"faith"}, {"faith"}),
            row("b5", "Call the bank", "", "", "", "", 15, 0, 0, {"learning"})};
    }
    void taskTagFilterScreens() {
        QTemporaryDir dir;
        const QJsonArray tasks{
            QJsonObject{{"id", "read"}, {"title", "Read Quran"}, {"project_id", "quran"},
                        {"epic_id", "e-juz"}, {"tag_ids", QJsonArray{"learning"}}},
            QJsonObject{{"id", "review"}, {"title", "Review yesterday’s reading"}, {"project_id", "quran"},
                        {"epic_id", "e-tajweed"}, {"tag_ids", QJsonArray{}}},
            QJsonObject{{"id", "walk"}, {"title", "Morning walk"}, {"tag_ids", QJsonArray{"learning"}}}};
        TrackingUiPreviewBackend backend(fake(dir), daemon_path(dir), tasks, backlogRows(),
                                         fixture("stats_table_tag_items"));
        Theme theme(dir.path());
        QQmlEngine engine;
        engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl("qrc:/qml/Main.qml"));
        QScopedPointer<QObject> root(component.create());
        QVERIFY2(root, qPrintable(component.errorString()));
        auto *window = qobject_cast<QQuickWindow *>(root.data());
        QVERIFY(window);
        window->resize(1280, 820);
        window->setProperty("page", 2);

        auto *stats = findByName(window, "statsScroll");
        QVERIFY(stats);
        auto *bar = stats->property("filterBar").value<QQuickItem *>();
        QVERIFY(bar);
        qreal previousY = -1;
        for (const char *name : {"statsRangeList", "statsProjectList", "statsEpicList",
                                 "statsTaskList", "statsTagList", "statsMetricList"}) {
            auto *list = qobject_cast<QQuickItem *>(findItemByName(bar, name));
            QVERIFY2(list && list->isVisible(), name);
            const qreal y = list->mapToItem(bar, QPointF()).y();
            QVERIFY2(y > previousY, name);
            previousY = y;
        }
        auto *search = findItemByName(bar, "statsTaskSearch");
        auto *allTasks = findItemByName(bar, "statsTaskAll");
        QVERIFY(search && allTasks);
        QCOMPARE(search->property("height").toReal(), allTasks->property("height").toReal());
        auto *searchBackground = search->property("background").value<QQuickItem *>();
        auto *chipBackground = allTasks->property("background").value<QQuickItem *>();
        QVERIFY(searchBackground && chipBackground);
        QCOMPARE(searchBackground->height(), chipBackground->height());
        QCOMPARE(bar->property("projectChoices").toList().size(), 3);
        auto click = [bar](const QString &name) {
            auto *chip = findItemByName(bar, name);
            return chip && chip->property("visible").toBool() && QMetaObject::invokeMethod(chip, "clicked");
        };
        QVERIFY(QMetaObject::invokeMethod(bar, "setFilter", Q_ARG(QVariant, QVariant("project")),
                                          Q_ARG(QVariant, QVariant("quran"))));
        QCOMPARE(bar->property("tagChoices").toList().size(), 1);
        QVERIFY(!findItemByName(bar, "statsTag_faith"));
        QVERIFY(click("statsTask_review"));
        QVERIFY(click("statsTag_learning"));
        QCOMPARE(bar->property("projectId").toString(), QString("quran"));
        QCOMPARE(bar->property("projectChoices").toList().size(), 3);
        QVERIFY(bar->property("itemId").toString().isEmpty());
        QCOMPARE(bar->property("taskChoices").toList().value(0).toMap().value("id").toString(), QString("read"));
        waitForBackendIdle(backend);
        backend.dismissError();
        shotWindow(window, "stats-task-tag-filters.png");
        QVERIFY(click("statsProject_none"));
        QVariant query;
        QVERIFY(QMetaObject::invokeMethod(bar, "query", Q_RETURN_ARG(QVariant, query)));
        QCOMPARE(query.toMap().value("tag_id").toString(), QString("learning"));
        QCOMPARE(query.toMap().value("project_id").toString(), QString("none"));
        QCOMPARE(bar->property("taskChoices").toList().value(0).toMap().value("id").toString(), QString("walk"));
        QVERIFY(QMetaObject::invokeMethod(bar, "setFilter", Q_ARG(QVariant, QVariant("project")),
                                          Q_ARG(QVariant, QVariant("quran"))));
        QVERIFY(click("statsTag_none"));
        QCOMPARE(bar->property("taskChoices").toList().value(0).toMap().value("id").toString(), QString("review"));
        QVERIFY(QMetaObject::invokeMethod(bar, "query", Q_RETURN_ARG(QVariant, query)));
        QCOMPARE(query.toMap().value("tag_id").toString(), QString("none"));
        QVERIFY(QMetaObject::invokeMethod(bar, "setFilter", Q_ARG(QVariant, QVariant("")),
                                          Q_ARG(QVariant, QVariant(""))));
        QVERIFY(QMetaObject::invokeMethod(stats, "setTab", Q_ARG(QVariant, QVariant("table"))));
        auto *table = findByName(window, "generalTable");
        QVERIFY(table);
        QVERIFY(QMetaObject::invokeMethod(table, "setGroup", Q_ARG(QVariant, QVariant("tag"))));
        const auto reportRows = table->property("rows").toList();
        QCOMPARE(reportRows.size(), 2);
        QCOMPARE(reportRows.last().toMap().value("sessions").toInt(), 1);
        QTRY_VERIFY(hasVisualText(window, "No tag"));
        window->resize(1280, 920);
        shotWindow(window, "stats-task-tag-table.png");
        window->resize(1280, 820);

        window->setProperty("page", 3);
        auto *settings = findByName(window, "settingsPage");
        QVERIFY(settings);
        settings->setProperty("section", "projects");
        auto *projects = findByName(window, "projectsSettings");
        QVERIFY(projects);
        QVERIFY(hasVisualText(window, "Default tags for new tasks"));
        waitForBackendIdle(backend);
        backend.dismissError();
        shotWindow(window, "settings-project-default-tags.png");
        settings->setProperty("section", "tags");
        QTRY_COMPARE(visualNamed(window, "tagUsage_learning").size(), 1);
        QCOMPARE(visualNamed(window, "tagUsage_learning").first()->property("text").toString(), QString("2 tasks · 1 project"));
        auto *faithUsage = visualNamed(window, "tagUsage_faith").value(0);
        QVERIFY(faithUsage);
        QCOMPARE(faithUsage->property("text").toString(), QString("0 tasks · 1 project"));
        shotWindow(window, "settings-task-tag-counts.png");

        window->setProperty("page", 5);
        QTRY_COMPARE(visualNamed(window, "backlogProjectDot_quran").size(), 1);
        auto *dot = visualNamed(window, "backlogProjectDot_quran").first();
        QCOMPARE(dot->property("color").value<QColor>(), QColor("#4c8bf5"));
        QCOMPARE(dot->property("width").toReal(), 10.0);
        QCOMPARE(dot->property("height").toReal(), 10.0);
        auto *studyTags = visualNamed(window, "backlogTaskTags_b1").value(0);
        auto *unassignedTags = visualNamed(window, "backlogTaskTags_b5").value(0);
        QVERIFY(studyTags && unassignedTags);
        QCOMPARE(studyTags->property("text").toString(), QString("Faith"));
        QCOMPARE(unassignedTags->property("text").toString(), QString("Learning"));
        shotWindow(window, "backlog-project-dots.png");
        auto *tag = visualNamed(window, "backlogTag_learning").value(0);
        auto *unassigned = visualNamed(window, "backlogProject_none").value(0);
        QVERIFY(unassigned && QMetaObject::invokeMethod(unassigned, "clicked"));
        QVERIFY(tag && QMetaObject::invokeMethod(tag, "clicked"));
        QVERIFY(unassigned->property("visible").toBool());
        QVERIFY(unassigned->property("selected").toBool());
    }
    void taskTagEditorWrites() {
        const QJsonObject added = fixture("item_add_tags");
        QVERIFY(Dto::decode<Dto::MutationResult>(added));
        ItemDraft canonical;
        canonical.update(added.value("changed").toObject().value("items").toArray().first().toObject());
        QCOMPARE(canonical.tagIds().toList(), QVariantList{"<id:1>"});
        QVERIFY(canonical.backlog() && canonical.projectId().isEmpty());
        const auto listed = rpcFixture("items_list").value("result").toArray();
        canonical.update(listed.first().toObject());
        QCOMPARE(canonical.tagIds().toList(), QVariantList{"<id:2>"});
        canonical.update(listed.last().toObject());
        QVERIFY(canonical.tagIds().isValid() && canonical.tagIds().toList().isEmpty());
        canonical.clear();
        QVERIFY(!canonical.tagIds().isValid());

        QTemporaryDir dir;
        TrackingUiPreviewBackend backend(fake(dir), daemon_path(dir), {}, backlogRows());
        Theme theme(dir.path());
        QQmlEngine engine;
        engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        ItemDraft backlogDraft;
        backlogDraft.update(backlogRows().first().toObject());
        QQmlComponent component(&engine, QUrl("qrc:/qml/Main.qml"));
        QScopedPointer<QObject> root(component.create());
        QVERIFY2(root, qPrintable(component.errorString()));
        auto *window = qobject_cast<QQuickWindow *>(root.data());
        QVERIFY(window);
        auto *entry = findByName(window, "entryPopup");
        QVERIFY(entry);
        auto *content = entry->property("contentItem").value<QQuickItem *>();
        QVERIFY(content);
        auto *title = findByName(window, "entryTitleField");
        QVERIFY(title);
        auto click = [content](const QString &name) {
            auto *chip = findItemByName(content, name);
            return chip && chip->property("visible").toBool() && QMetaObject::invokeMethod(chip, "clicked");
        };
        auto selected = [content](const QString &name) {
            auto *chip = findItemByName(content, "entryTag_" + name);
            return chip && chip->property("selected").toBool();
        };
        auto submit = [entry](bool keepOpen = false) {
            return QMetaObject::invokeMethod(entry, "submitTitle", Q_ARG(QVariant, QVariant(keepOpen)));
        };
        backend.reload();
        waitForBackendIdle(backend);
        QVERIFY(QMetaObject::invokeMethod(window, "openEntry", Q_ARG(QVariant, QVariant(QVariantMap{}))));
        QTRY_VERIFY(entry->property("opened").toBool());
        QVERIFY(click("entryProject_quran"));
        QVERIFY(selected("faith"));
        QVERIFY(click("entryTag_health") && selected("health"));
        QVERIFY(click("entryProject_study"));
        QVERIFY(selected("learning") && !selected("faith") && !selected("health"));
        title->setProperty("text", "Read chapter");
        shotWindow(window, "task-editor-default-tags.png");
        QVERIFY(submit());
        QTRY_COMPARE(offlineRequests(dir).size(), 1);
        auto params = offlineRequests(dir).last().value("params").toObject();
        QCOMPARE(params.value("project_id").toString(), QString("study"));
        QVERIFY(!params.contains("tag_ids"));

        window->setProperty("page", 5);
        QVERIFY(QMetaObject::invokeMethod(window, "openBacklogEntry"));
        QTRY_VERIFY(entry->property("opened").toBool());
        QVERIFY(click("entryProject_quran") && selected("faith"));
        QVERIFY(click("entryTag_faith") && !selected("faith"));
        title->setProperty("text", "Untagged backlog task");
        QVERIFY(submit(true));
        QTRY_COMPARE(offlineRequests(dir).size(), 2);
        params = offlineRequests(dir).last().value("params").toObject();
        QCOMPARE(params.value("tag_ids"), QJsonValue(QJsonArray{}));
        QVERIFY(params.value("backlog").toBool());
        QVERIFY(selected("faith"));
        QVERIFY(click("noProjectChip"));
        QVERIFY(click("entryTag_learning") && selected("learning"));
        title->setProperty("text", "Read without a project");
        QVERIFY(submit());
        QTRY_COMPARE(offlineRequests(dir).size(), 3);
        params = offlineRequests(dir).last().value("params").toObject();
        QCOMPARE(params.value("tag_ids"), QJsonValue(QJsonArray{"learning"}));
        QVERIFY(!params.contains("project_id"));

        window->setProperty("editItem", QVariant::fromValue(&backlogDraft));
        window->setProperty("entryVisible", true);
        QTRY_VERIFY(entry->property("opened").toBool());
        QVERIFY(selected("faith") && !selected("learning"));
        shotWindow(window, "backlog-editor-task-tags.png");
        QVERIFY(click("entryProject_fitness") && selected("faith") && !selected("health"));
        QVERIFY(click("entryTag_faith") && !selected("faith"));
        const auto openedVersion = backlogDraft.version();
        auto newer = backlogRows().first().toObject();
        newer["version"] = openedVersion + 1;
        newer["tag_ids"] = QJsonArray{"faith", "learning"};
        backlogDraft.update(newer);
        QVERIFY(submit());
        QTRY_COMPARE(offlineRequests(dir).size(), 4);
        QCOMPARE(offlineRequests(dir).last().value("method").toString(), QString("item.edit"));
        params = offlineRequests(dir).last().value("params").toObject();
        QCOMPARE(params.value("id").toString(), QString("b1"));
        QCOMPARE(params.value("expected_version").toInteger(), openedVersion);
        QCOMPARE(params.value("project_id").toString(), QString("fitness"));
        QCOMPARE(params.value("tag_ids"), QJsonValue(QJsonArray{}));
    }
    void backlogRequestsFollowCanonicalFixtures() {
        for (const char *name : {"item_add_backlog", "item_schedule", "item_backlog"})
            QVERIFY2(Dto::decode<Dto::MutationResult>(fixture(name)), name);
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        auto state = trackingState();
        state->backlog = backlogRows();
        daemon.setHandler(trackingHandler(state));
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        backend.reload();
        waitForBackendIdle(backend);
        backend.loadBacklog("", "study");
        QTRY_COMPARE_WITH_TIMEOUT(backend.backlogItems().size(), 3, 3000);
        QCOMPARE(lastRequest(daemon, "backlog.list").value("params").toObject().value("project_id").toString(), QString("study"));
        backend.loadBacklog("", "");
        QTRY_COMPARE_WITH_TIMEOUT(backend.backlogItems().size(), 5, 3000);

        backend.scheduleItem("b1", "2031-05-15", "asr");
        QTRY_VERIFY_WITH_TIMEOUT(!lastRequest(daemon, "item.schedule").isEmpty(), 3000);
        QJsonObject params = lastRequest(daemon, "item.schedule").value("params").toObject();
        QCOMPARE(params.value("id").toString(), QString("b1"));
        QCOMPARE(params.value("day").toString(), QString("2031-05-15"));
        QCOMPARE(params.value("block").toString(), QString("asr"));
        QVERIFY(params.contains("expected_version") && params.contains("op_id"));

        backend.moveBacklogItem("b2", "b1", true);
        QTRY_VERIFY_WITH_TIMEOUT(!lastRequest(daemon, "item.move").isEmpty(), 3000);
        params = lastRequest(daemon, "item.move").value("params").toObject();
        QVERIFY(params.value("block").isNull());
        QCOMPARE(params.value("before_id").toString(), QString("b1"));
        QVERIFY(!params.contains("after_id"));

        backend.sendToBacklog("deep");
        QTRY_VERIFY_WITH_TIMEOUT(!lastRequest(daemon, "item.backlog").isEmpty(), 3000);
        QCOMPARE(lastRequest(daemon, "item.backlog").value("params").toObject().value("id").toString(), QString("deep"));

        backend.saveItem(QVariantMap{{"title", "Plan trip"}, {"backlog", true}, {"repeat", "none"}});
        QTRY_VERIFY_WITH_TIMEOUT(!lastRequest(daemon, "item.add").isEmpty(), 3000);
        params = lastRequest(daemon, "item.add").value("params").toObject();
        QVERIFY(params.value("backlog").toBool());
        QVERIFY(!params.contains("block") && !params.contains("date"));
        QVERIFY2(backend.error().isEmpty(), qPrintable(backend.error()));
    }
    void backlogScreens() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        auto state = trackingState();
        state->backlog = backlogRows();
        {
            // One one-time item in view: only those offer "-> Backlog".
            QJsonArray blocks = state->day.value("blocks").toArray();
            for (int b = 0; b < blocks.size(); ++b) {
                QJsonObject block = blocks.at(b).toObject();
                QJsonArray items = block.value("items").toArray();
                for (int i = 0; i < items.size(); ++i) {
                    QJsonObject item = items.at(i).toObject();
                    if (item.value("id").toString() == "dhikr") { item["repeat"] = "none"; items[i] = item; }
                }
                block["items"] = items; blocks[b] = block;
            }
            state->day["blocks"] = blocks;
        }
        daemon.setHandler(trackingHandler(state));
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        Theme theme(dir.path());
        QQmlEngine engine; engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl::fromLocalFile(QStringLiteral(TEST_SOURCE_DIR "/../src/qml/Main.qml")));
        QScopedPointer<QObject> root(component.create()); QVERIFY2(root, qPrintable(component.errorString()));
        auto *window = qobject_cast<QQuickWindow *>(root.data()); QVERIFY(window);
        window->setWidth(1440); window->setHeight(900); window->show();
        backend.reload();
        waitForBackendIdle(backend);
        shotWindow(window, "today-to-backlog.png");
        bool anyToBacklog = false;
        for (QObject *button : visualNamed(window, "toBacklogButton"))
            anyToBacklog = anyToBacklog || button->property("visible").toBool();
        QVERIFY(anyToBacklog);

        root->setProperty("page", 5);
        QTRY_COMPARE_WITH_TIMEOUT(backend.backlogItems().size(), 5, 3000);
        shotWindow(window, "backlog-page.png");
        QObject *list = findByName(root.data(), "backlogTaskList"); QVERIFY(list);
        // 3 project headings + 2 epic headings + (study has one epic-less group) + 5 tasks.
        QVERIFY(list->property("count").toInt() >= 10);
        QVERIFY(!visualNamed(window, "backlogHandle_b1").isEmpty());
        QObject *project = visualNamed(window, "backlogProject_quran").value(0); QVERIFY(project);
        QVERIFY(QMetaObject::invokeMethod(project, "clicked"));
        QTRY_COMPARE_WITH_TIMEOUT(backend.backlogItems().size(), 1, 3000);
        QCOMPARE(lastRequest(daemon, "backlog.list").value("params").toObject().value("project_id").toString(), QString("quran"));
        shotWindow(window, "backlog-filtered.png");
        QVERIFY(QMetaObject::invokeMethod(visualNamed(window, "backlogProject_").value(0), "clicked"));
        QTRY_COMPARE_WITH_TIMEOUT(backend.backlogItems().size(), 5, 3000);
        QVERIFY(QMetaObject::invokeMethod(visualNamed(window, "backlogTag_learning").value(0), "clicked"));
        QTRY_COMPARE_WITH_TIMEOUT(backend.backlogItems().size(), 2, 3000);
        QCOMPARE(backend.backlogItems().first().toMap().value("id").toString(), QString("b2"));
        QCOMPARE(backend.backlogItems().last().toMap().value("id").toString(), QString("b5"));
        QVERIFY(QMetaObject::invokeMethod(visualNamed(window, "backlogProject_none").value(0), "clicked"));
        QTRY_COMPARE_WITH_TIMEOUT(backend.backlogItems().size(), 1, 3000);
        QCOMPARE(backend.backlogItems().first().toMap().value("id").toString(), QString("b5"));
        QCOMPARE(lastRequest(daemon, "backlog.list").value("params").toObject(),
                 (QJsonObject{{"project_id", "none"}, {"tag_id", "learning"}}));
        shotWindow(window, "backlog-filtered-task-tag.png");
        QVERIFY(QMetaObject::invokeMethod(visualNamed(window, "backlogProject_").value(0), "clicked"));
        QVERIFY(QMetaObject::invokeMethod(visualNamed(window, "backlogTag_none").value(0), "clicked"));
        QTRY_COMPARE_WITH_TIMEOUT(backend.backlogItems().value(0).toMap().value("id").toString(), QString("b3"), 3000);
        QVERIFY(QMetaObject::invokeMethod(visualNamed(window, "backlogTag_").value(0), "clicked"));
        QTRY_COMPARE_WITH_TIMEOUT(backend.backlogItems().size(), 5, 3000);

        QObject *dialog = findByName(root.data(), "scheduleDialog"); QVERIFY(dialog);
        QVERIFY(QMetaObject::invokeMethod(root.data(), "openSchedule", Q_ARG(QVariant, backend.backlogItems().first())));
        QTRY_VERIFY_WITH_TIMEOUT(dialog->property("visible").toBool(), 2000);
        QObject *tomorrow = findItemByName(dialog->property("contentItem").value<QQuickItem *>(), "scheduleTomorrow"); QVERIFY(tomorrow);
        QVERIFY(QMetaObject::invokeMethod(tomorrow, "clicked"));
        shotWindow(window, "schedule-dialog.png");
        QObject *dateChip = findItemByName(dialog->property("contentItem").value<QQuickItem *>(), "scheduleDate");
        QVERIFY(QMetaObject::invokeMethod(dateChip, "clicked"));
        shotWindow(window, "schedule-dialog-date.png");
        QVERIFY(QMetaObject::invokeMethod(tomorrow, "clicked"));
        QObject *asr = findItemByName(dialog->property("contentItem").value<QQuickItem *>(), "scheduleBlock_asr");
        QVERIFY(QMetaObject::invokeMethod(asr, "clicked"));
        QVERIFY(dialog->property("canSave").toBool());
        QVERIFY(QMetaObject::invokeMethod(findItemByName(dialog->property("contentItem").value<QQuickItem *>(), "scheduleConfirm"), "clicked"));
        QTRY_VERIFY_WITH_TIMEOUT(!lastRequest(daemon, "item.schedule").isEmpty(), 3000);
        const QJsonObject params = lastRequest(daemon, "item.schedule").value("params").toObject();
        QCOMPARE(params.value("block").toString(), QString("asr"));
        QCOMPARE(params.value("day").toString(), dialog->property("tomorrowText").toString());
        QTRY_VERIFY_WITH_TIMEOUT(!dialog->property("visible").toBool(), 3000);
        QVERIFY2(backend.error().isEmpty(), qPrintable(backend.error()));
        window->hide();
    }
    void trackingUiScreens() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        auto state = trackingState();
        daemon.setHandler(trackingHandler(state));
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        Theme theme(dir.path());
        QQmlEngine engine; engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl::fromLocalFile(QStringLiteral(TEST_SOURCE_DIR "/../src/qml/Main.qml")));
        QScopedPointer<QObject> root(component.create()); QVERIFY2(root, qPrintable(component.errorString()));
        auto *window = qobject_cast<QQuickWindow *>(root.data()); QVERIFY(window);
        window->setWidth(1440); window->setHeight(900); window->show();
        backend.reload();
        waitForBackendIdle(backend);
        QTRY_COMPARE_WITH_TIMEOUT(backend.projects().size(), 4, 3000);
        QTRY_COMPARE_WITH_TIMEOUT(backend.epics().size(), 5, 3000);
        shotWindow(window, "today.png");

        QObject *settings = findByName(root.data(), "settingsPage"); QVERIFY(settings);
        root->setProperty("page", 3);
        settings->setProperty("section", "general");
        shotWindow(window, "settings-general.png");
        QVERIFY(hasLabelText(settings, "Sleep time"));
        QVERIFY(hasLabelText(settings, "Prayer calculation"));
        settings->setProperty("section", "projects");
        QObject *projectsPane = findByName(root.data(), "projectsSettings"); QVERIFY(projectsPane);
        shotWindow(window, "settings-projects-new.png");
        const QVariant quran = backend.projects().first();
        QVERIFY(QMetaObject::invokeMethod(projectsPane, "startEdit", Q_ARG(QVariant, quran)));
        shotWindow(window, "settings-projects-edit.png");
        QCOMPARE(projectsPane->property("editId").toString(), QString("quran"));
        // Settings -> Projects lists the project's epics; only an epic without
        // sessions can be removed, the rest are finished.
        QObject *epicList = findByName(root.data(), "projectEpics"); QVERIFY(epicList);
        QCOMPARE(epicList->property("rows").toList().size(), 2);
        shotWindow(window, "settings-epics-quran.png");
        QVERIFY(QMetaObject::invokeMethod(projectsPane, "startEdit", Q_ARG(QVariant, backend.projects().at(1))));
        QCOMPARE(epicList->property("rows").toList().size(), 3);
        int removable = 0;
        for (QObject *object : visualNamed(window, "epicRemoveButton"))
            if (object->property("visible").toBool()) ++removable;
        QCOMPARE(removable, 1);
        shotWindow(window, "settings-epics-study.png");
        QObject *emptyRow = visualNamed(window, "epicRow_e-empty").value(0); QVERIFY(emptyRow);
        QObject *finish = findItemByName(emptyRow, "epicStatusButton"); QVERIFY(finish);
        QCOMPARE(finish->property("text").toString(), QString("Finish"));
        QVERIFY(QMetaObject::invokeMethod(finish, "clicked"));
        QTRY_VERIFY_WITH_TIMEOUT(!lastRequest(daemon, "epic.edit").isEmpty(), 3000);
        QCOMPARE(lastRequest(daemon, "epic.edit").value("params").toObject().value("status").toString(), QString("finished"));
        {
            // Rename through the UI: the field is filled, Save sends epic.edit with the new name.
            QVERIFY(QMetaObject::invokeMethod(findItemByName(emptyRow, "epicRenameButton"), "clicked"));
            QObject *field = findItemByName(emptyRow, "epicRenameField"); QVERIFY(field);
            QCOMPARE(field->property("text").toString(), QString("Reading list"));
            field->setProperty("text", "Reading shelf");
            QVERIFY(QMetaObject::invokeMethod(findItemByName(emptyRow, "epicRenameSave"), "clicked"));
            QTRY_COMPARE_WITH_TIMEOUT(lastRequest(daemon, "epic.edit").value("params").toObject().value("name").toString(),
                                      QString("Reading shelf"), 3000);
        }
        QVERIFY(QMetaObject::invokeMethod(findItemByName(emptyRow, "epicRemoveButton"), "clicked"));
        QVERIFY(QMetaObject::invokeMethod(findItemByName(emptyRow, "epicRemoveConfirm"), "clicked"));
        QTRY_VERIFY_WITH_TIMEOUT(!lastRequest(daemon, "epic.remove").isEmpty(), 3000);
        QCOMPARE(lastRequest(daemon, "epic.remove").value("params").toObject().value("id").toString(), QString("e-empty"));
        settings->setProperty("section", "tags");
        QTRY_COMPARE_WITH_TIMEOUT(backend.trackingItems().size(), 6, 3000);
        QTRY_COMPARE(visualNamed(window, "tagUsage_learning").size(), 1);
        QCOMPARE(visualNamed(window, "tagUsage_learning").first()->property("text").toString(), QString("3 tasks · 1 project"));
        shotWindow(window, "settings-tags.png");
        settings->setProperty("section", "metrics");
        QObject *metricsPane = findByName(root.data(), "metricsSettings"); QVERIFY(metricsPane);
        QVERIFY(QMetaObject::invokeMethod(metricsPane, "startEdit", Q_ARG(QVariant, backend.metrics().at(2))));
        shotWindow(window, "settings-metrics.png");

        root->setProperty("page", 0);
        QVERIFY(QMetaObject::invokeMethod(root.data(), "openEntry", Q_ARG(QVariant, QVariant("deep"))));
        shotWindow(window, "item-editor.png");
        {
            // Epic chips: only the project's active epics, "No epic" by default
            // for a task without one; the saved draft carries or clears epic_id.
            QObject *entry = findByName(root.data(), "entryPopup"); QVERIFY(entry);
            QCOMPARE(entry->property("epicId").toString(), QString("e-grammar"));
            QCOMPARE(entry->property("epicChoices").toList().size(), 2);
            auto *content = entry->property("contentItem").value<QQuickItem *>(); QVERIFY(content);
            auto chip = [&](const QString &name) -> QObject * {
                for (QQuickItem *child : quickDescendants(content)) if (child->objectName() == name) return child;
                return nullptr;
            };
            QVERIFY(chip("noEpicChip") && chip("epicChip_e-grammar") && chip("epicChip_e-empty"));
            QVERIFY(!chip("epicChip_e-old"));
            shotWindow(window, "item-editor-epics.png");
            QVERIFY(QMetaObject::invokeMethod(chip("epicChip_e-empty"), "clicked"));
            QVariant draft;
            QVERIFY(QMetaObject::invokeMethod(entry, "draft", Q_RETURN_ARG(QVariant, draft)));
            QCOMPARE(draft.toMap().value("epic_id").toString(), QString("e-empty"));
            QVERIFY(QMetaObject::invokeMethod(chip("noEpicChip"), "clicked"));
            QVERIFY(QMetaObject::invokeMethod(entry, "draft", Q_RETURN_ARG(QVariant, draft)));
            QVERIFY(draft.toMap().contains("epic_id"));
            QCOMPARE(draft.toMap().value("epic_id").toString(), QString());
            // Another project: its own epics, and none chosen.
            QVERIFY(QMetaObject::invokeMethod(entry, "selectProject", Q_ARG(QVariant, QVariant("quran"))));
            QCOMPARE(entry->property("epicChoices").toList().size(), 2);
            QCOMPARE(entry->property("epicId").toString(), QString());
            QVERIFY(QMetaObject::invokeMethod(entry, "selectProject", Q_ARG(QVariant, QVariant("study"))));
            QVERIFY(QMetaObject::invokeMethod(chip("epicChip_e-grammar"), "clicked"));
            QCOMPARE(entry->property("epicId").toString(), QString("e-grammar"));
        }
        {
            QObject *entry = findByName(root.data(), "entryPopup");
            QObject *form = entry ? entry : nullptr;
            if (form) QVERIFY(QMetaObject::invokeMethod(form, "toggleMetric", Q_ARG(QVariant, QVariant("quality"))));
            shotWindow(window, "item-editor-asks.png");
        }
        root->setProperty("entryVisible", false);

        // Today with the session lists of a timed, a ticked and a running item open.
        QObject *timeline = nullptr;
        for (QObject *object : root->findChildren<QObject *>())
            if (object->property("expanded").isValid() && object->metaObject()->indexOfMethod("toggleExpanded(QVariant)") >= 0) timeline = object;
        QVERIFY(timeline);
        for (const char *id : {"quran", "walk", "deep"})
            QVERIFY(QMetaObject::invokeMethod(timeline, "toggleExpanded", Q_ARG(QVariant, QVariant(id))));
        shotWindow(window, "today-sessions.png");
        // A session row names its epic.
        QVERIFY(hasVisualText(window, "Memorise Juz 30"));
        shotWindow(window, "session-notes.png");
        QQuickItem *list = timelineView(root.data()); QVERIFY(list);
        list->setProperty("contentY", 420);
        shotWindow(window, "today-sessions-scrolled.png");
        list->setProperty("contentY", 0);
        QObject *runningCard = findByName(root.data(), "runningCard"); QVERIFY(runningCard);
        QVERIFY(runningCard->property("visible").toBool());

        QObject *prompt = findByName(root.data(), "sessionPrompt"); QVERIFY(prompt);
        auto showPrompt = [&](const QVariantMap &options) {
            return QMetaObject::invokeMethod(prompt, "show", Q_ARG(QVariant, QVariant(options)));
        };
        // The running card with distractions and Add note open.
        QObject *addNote = findByName(root.data(), "runningAddNoteButton"); QVERIFY(addNote);
        QVERIFY(QMetaObject::invokeMethod(addNote, "clicked"));
        QTRY_VERIFY_WITH_TIMEOUT(runningCard->property("addingNote").toBool(), 2000);
        QObject *noteField = findByName(root.data(), "runningNoteField"); QVERIFY(noteField);
        noteField->setProperty("text", "Reading the dative chapter");
        QCOMPARE(findByName(root.data(), "runningDistractions")->property("text").toString(), QString("Distractions 3"));
        shotWindow(window, "running-card.png");
        QVERIFY(QMetaObject::invokeMethod(noteField, "accepted"));
        QTRY_VERIFY_WITH_TIMEOUT(!lastRequest(daemon, "session.note").isEmpty(), 3000);
        QCOMPARE(lastRequest(daemon, "session.note").value("params").toObject().value("text").toString(), QString("Reading the dative chapter"));
        QVERIFY(QMetaObject::invokeMethod(findByName(root.data(), "distractionPlus"), "clicked"));
        QTRY_VERIFY_WITH_TIMEOUT(!lastRequest(daemon, "session.bump").isEmpty(), 3000);
        QCOMPARE(lastRequest(daemon, "session.bump").value("params").toObject().value("delta").toInt(), 1);
        QObject *pause = findByName(root.data(), "runningStopButton"); QVERIFY(pause);
        QVERIFY(QMetaObject::invokeMethod(pause, "clicked"));
        QTRY_VERIFY_WITH_TIMEOUT(prompt->property("visible").toBool(), 2000);
        {
            bool found = false;
            for (QQuickItem *item : quickDescendants(prompt->property("contentItem").value<QQuickItem *>()))
                if (item->objectName() == "promptDistractions") found = true;
            QVERIFY(found);
        }
        shotWindow(window, "prompt-pause.png");
        root->setProperty("sessionPromptVisible", false);

        // The done panel: the project's metrics, all optional, Done and Skip.
        QVERIFY(showPrompt(QVariantMap{
            {"mode", "done"}, {"itemId", "dhikr"}, {"projectId", "quran"}, {"title", "Dhikr after Dhuhr"}, {"color", "#4c8bf5"}}));
        QTRY_VERIFY_WITH_TIMEOUT(prompt->property("visible").toBool(), 2000);
        QCOMPARE(prompt->property("metrics").toList().size(), 4);
        QVERIFY(prompt->property("canSave").toBool());
        QVERIFY(findByName(root.data(), "sessionSkipButton")->property("visible").toBool());
        shotWindow(window, "prompt-done.png");
        root->setProperty("sessionPromptVisible", false);

        QVERIFY(showPrompt(QVariantMap{
            {"mode", "log"}, {"itemId", "quran"}, {"projectId", "quran"}, {"title", "Read Quran"}, {"color", "#4c8bf5"}}));
        QTRY_VERIFY_WITH_TIMEOUT(prompt->property("visible").toBool(), 2000);
        // Quran asks Focus, Quality, Pages read and Memorised; minutes are needed to save.
        const QVariantList metrics = prompt->property("metrics").toList();
        QCOMPARE(metrics.size(), 4);
        QCOMPARE(metrics.at(2).toMap().value("metric_id").toString(), QString("pages"));
        QVERIFY(!prompt->property("canSave").toBool());
        shotWindow(window, "prompt-log.png");
        root->setProperty("sessionPromptVisible", false);

        QVERIFY(showPrompt(QVariantMap{
            {"mode", "edit"}, {"sessionId", "s-q1"}, {"projectId", "quran"}, {"title", "Read Quran"},
            {"color", "#4c8bf5"}, {"minutes", 25}, {"note", "Calm morning"},
            {"values", QVariantMap{{"focus", 4}, {"quality", 5}, {"pages", 6}}}}));
        QTRY_VERIFY_WITH_TIMEOUT(prompt->property("visible").toBool(), 2000);
        QVERIFY(prompt->property("canSave").toBool());
        shotWindow(window, "prompt-edit.png");
        root->setProperty("sessionPromptVisible", false);

        // A time log without a task can name one of the project's epics.
        QVERIFY(showPrompt(QVariantMap{
            {"mode", "log"}, {"projectId", "study"}, {"title", "Study"}, {"color", "#9b59b6"}}));
        QTRY_VERIFY_WITH_TIMEOUT(prompt->property("visible").toBool(), 2000);
        QCOMPARE(prompt->property("epicChoices").toList().size(), 2);
        QCOMPARE(prompt->property("epicId").toString(), QString());
        findByName(root.data(), "sessionMinutesField")->setProperty("text", "20");
        shotWindow(window, "prompt-log-epic.png");
        prompt->setProperty("epicId", "e-grammar");
        QVERIFY(QMetaObject::invokeMethod(prompt, "save", Q_ARG(QVariant, QVariant(false)), Q_ARG(QVariant, QVariant(false))));
        QTRY_VERIFY_WITH_TIMEOUT(!lastRequest(daemon, "session.log").isEmpty(), 3000);
        QCOMPARE(lastRequest(daemon, "session.log").value("params").toObject().value("epic_id").toString(), QString("e-grammar"));
        QVERIFY(!lastRequest(daemon, "session.log").value("params").toObject().contains("item_id"));
        QTRY_VERIFY_WITH_TIMEOUT(!prompt->property("visible").toBool(), 3000);
        root->setProperty("sessionPromptVisible", false);

        QVERIFY(showPrompt(QVariantMap{
            {"mode", "log"}, {"itemId", "walk"}, {"title", "Morning walk"}}));
        QTRY_VERIFY_WITH_TIMEOUT(prompt->property("visible").toBool(), 2000);
        // No project: the built-in Focus, Quality and Distractions.
        QCOMPARE(prompt->property("metrics").toList().size(), 3);
        QCOMPARE(prompt->property("metrics").toList().first().toMap().value("metric_id").toString(), QString("focus"));
        root->setProperty("sessionPromptVisible", false);
        // Statistics: all, one project, 90 days, scrolled to the breakdowns.
        root->setProperty("page", 2);
        QVERIFY(QMetaObject::invokeMethod(root.data(), "loadStatistics"));
        waitForBackendIdle(backend);
        shotWindow(window, "stats-all.png");
        QObject *stats = findByName(root.data(), "statsScroll"); QVERIFY(stats);
        QObject *flick = stats->property("contentItem").value<QObject *>(); QVERIFY(flick);
        trendsScreens(window, stats);
        waitForBackendIdle(backend);
        {
            // Trends can split by epic; the unassigned work is its own line.
            auto *trends = qobject_cast<QQuickItem *>(findByName(stats, "trendsChart")); QVERIFY(trends);
            QVERIFY(QMetaObject::invokeMethod(findItemByName(trends, "trendsSplit_epic"), "clicked"));
            waitForBackendIdle(backend);
            QCOMPARE(lastRequest(daemon, "stats.series").value("params").toObject().value("split").toString(), QString("epic"));
            QTRY_COMPARE_WITH_TIMEOUT(backend.trends().value("split").toString(), QString("epic"), 3000);
            flick->setProperty("contentY", flick->property("contentY").toReal()
                               + trends->mapToItem(qobject_cast<QQuickItem *>(flick), QPointF()).y() - 10);
            shotWindow(window, "trends-epic-split.png");
            QVERIFY(QMetaObject::invokeMethod(findItemByName(trends, "trendsSplit_"), "clicked"));
            waitForBackendIdle(backend);
            flick->setProperty("contentY", 0);
        }
        flick->setProperty("contentY", flick->property("contentHeight").toReal() - flick->property("height").toReal());
        shotWindow(window, "stats-all-bottom.png");
        flick->setProperty("contentY", 0);
        QTRY_VERIFY_WITH_TIMEOUT(!backend.trackingItems().isEmpty(), 3000);
        QVERIFY(QMetaObject::invokeMethod(stats, "setFilter", Q_ARG(QVariant, QVariant("tag")), Q_ARG(QVariant, QVariant("learning"))));
        waitForBackendIdle(backend);
        QCOMPARE(lastRequest(daemon, "stats.get").value("params").toObject().value("tag_id").toString(), QString("learning"));
        // The project choices stay available while a tag is selected.
        QCOMPARE(stats->property("projectChoices").toList().size(), 3);
        QVERIFY(QMetaObject::invokeMethod(stats, "setFilter", Q_ARG(QVariant, QVariant("project")), Q_ARG(QVariant, QVariant("study"))));
        waitForBackendIdle(backend);
        QCOMPARE(stats->property("taskChoices").toList().size(), 2);
        shotWindow(window, "stats-filters.png");
        QVERIFY(QMetaObject::invokeMethod(stats, "setFilter", Q_ARG(QVariant, QVariant("item")), Q_ARG(QVariant, QVariant("deep"))));
        waitForBackendIdle(backend);
        QCOMPARE(lastRequest(daemon, "stats.get").value("params").toObject().value("item_id").toString(), QString("deep"));
        QVERIFY(QMetaObject::invokeMethod(stats, "setFilter", Q_ARG(QVariant, QVariant("")), Q_ARG(QVariant, QVariant(""))));
        waitForBackendIdle(backend);
        // One chart in Table mode.
        QObject *dayChart = findByName(root.data(), "dayChart"); QVERIFY(dayChart);
        QVERIFY(QMetaObject::invokeMethod(findItemByName(dayChart, "chartTableButton"), "clicked"));
        QTRY_VERIFY_WITH_TIMEOUT(dayChart->property("tableMode").toBool(), 2000);
        auto *dayItem = qobject_cast<QQuickItem *>(dayChart);
        auto *viewport = qobject_cast<QQuickItem *>(flick);
        QVERIFY(dayItem && viewport);
        flick->setProperty("contentY", dayItem->mapToItem(viewport, QPointF()).y() + flick->property("contentY").toReal());
        shotWindow(window, "stats-chart-table.png");
        flick->setProperty("contentY", 0);
        QVERIFY(QMetaObject::invokeMethod(findItemByName(dayChart, "chartChartButton"), "clicked"));
        QVERIFY(!dayChart->property("tableMode").toBool());
        QVERIFY(QMetaObject::invokeMethod(stats, "setFilter", Q_ARG(QVariant, QVariant("project")), Q_ARG(QVariant, QVariant("quran"))));
        waitForBackendIdle(backend);
        QCOMPARE(lastRequest(daemon, "stats.get").value("params").toObject().value("project_id").toString(), QString("quran"));
        shotWindow(window, "stats-project.png");
        QVERIFY(QMetaObject::invokeMethod(stats, "setFilter", Q_ARG(QVariant, QVariant("")), Q_ARG(QVariant, QVariant(""))));
        QVERIFY(QMetaObject::invokeMethod(stats, "setRange", Q_ARG(QVariant, QVariant(90))));
        waitForBackendIdle(backend);
        QCOMPARE(lastRequest(daemon, "stats.get").value("params").toObject().value("days").toInt(), 90);
        shotWindow(window, "stats-90.png");

        // Epics row: narrowed by the project, with "No epic" for the rest.
        QVERIFY(QMetaObject::invokeMethod(stats, "setFilter", Q_ARG(QVariant, QVariant("project")), Q_ARG(QVariant, QVariant("study"))));
        waitForBackendIdle(backend);
        QVERIFY(findByName(root.data(), "statsEpicList"));
        QCOMPARE(stats->property("filterBar").value<QObject *>()->property("epicChoices").toList().size(), 3);
        QVERIFY(QMetaObject::invokeMethod(stats, "setFilter", Q_ARG(QVariant, QVariant("epic")), Q_ARG(QVariant, QVariant("e-grammar"))));
        waitForBackendIdle(backend);
        QCOMPARE(lastRequest(daemon, "stats.get").value("params").toObject().value("epic_id").toString(), QString("e-grammar"));
        QCOMPARE(stats->property("taskChoices").toList().size(), 1);
        shotWindow(window, "stats-epics-filter.png");
        QVERIFY(QMetaObject::invokeMethod(stats, "setFilter", Q_ARG(QVariant, QVariant("epic")), Q_ARG(QVariant, QVariant("none"))));
        waitForBackendIdle(backend);
        QCOMPARE(lastRequest(daemon, "stats.get").value("params").toObject().value("epic_id").toString(), QString("none"));
        QVERIFY(QMetaObject::invokeMethod(stats, "setFilter", Q_ARG(QVariant, QVariant("")), Q_ARG(QVariant, QVariant(""))));
        waitForBackendIdle(backend);
        QVERIFY(!lastRequest(daemon, "stats.get").value("params").toObject().contains("epic_id"));
        flick->setProperty("contentY", flick->property("contentHeight").toReal() - flick->property("height").toReal());
        shotWindow(window, "stats-epic-breakdown.png");
        flick->setProperty("contentY", 0);

        // The shared filter bar with two metric chips, on each tab.
        QVERIFY(QMetaObject::invokeMethod(stats, "setRange", Q_ARG(QVariant, QVariant(28))));
        QObject *bar = stats->property("filterBar").value<QObject *>(); QVERIFY(bar);
        QVERIFY(QMetaObject::invokeMethod(bar, "setMetricRange", Q_ARG(QVariant, QVariant("focus")), Q_ARG(QVariant, QVariant(4)), Q_ARG(QVariant, QVariant(""))));
        QVERIFY(QMetaObject::invokeMethod(bar, "setMetricRange", Q_ARG(QVariant, QVariant("distractions")), Q_ARG(QVariant, QVariant("")), Q_ARG(QVariant, QVariant(2))));
        waitForBackendIdle(backend);
        QCOMPARE(lastRequest(daemon, "stats.get").value("params").toObject().value("values").toObject().size(), 2);
        shotWindow(window, "stats-charts-metrics.png");
        QVERIFY(QMetaObject::invokeMethod(bar, "openEditor", Q_ARG(QVariant, QVariant("focus"))));
        shotWindow(window, "stats-metric-editor.png");
        bar->setProperty("editing", "");
        QVERIFY(QMetaObject::invokeMethod(stats, "setTab", Q_ARG(QVariant, QVariant("table"))));
        waitForBackendIdle(backend);
        QCOMPARE(lastRequest(daemon, "stats.table").value("params").toObject().value("values").toObject().size(), 2);
        shotWindow(window, "stats-table-week.png");
        QObject *table = findByName(root.data(), "generalTable"); QVERIFY(table);
        QVERIFY(QMetaObject::invokeMethod(table, "setGroup", Q_ARG(QVariant, QVariant("project"))));
        waitForBackendIdle(backend);
        QVERIFY(QMetaObject::invokeMethod(table, "toggleColumn", Q_ARG(QVariant, QVariant("quality"))));
        QVERIFY(QMetaObject::invokeMethod(table, "sortBy", Q_ARG(QVariant, QVariant("minutes"))));
        shotWindow(window, "stats-table-project.png");
        QVERIFY(QMetaObject::invokeMethod(table, "toggleColumn", Q_ARG(QVariant, QVariant("sessions"))));
        QVERIFY(QMetaObject::invokeMethod(table, "setGroup", Q_ARG(QVariant, QVariant("tag"))));
        waitForBackendIdle(backend);
        shotWindow(window, "stats-table-tag-no-sessions.png");
        QVERIFY(QMetaObject::invokeMethod(table, "setGroup", Q_ARG(QVariant, QVariant("epic"))));
        waitForBackendIdle(backend);
        QCOMPARE(lastRequest(daemon, "stats.table").value("params").toObject().value("group").toString(), QString("epic"));
        shotWindow(window, "stats-table-epic.png");
        QVERIFY(QMetaObject::invokeMethod(table, "toggleColumn", Q_ARG(QVariant, QVariant("sessions"))));
        QVERIFY(QMetaObject::invokeMethod(table, "toggleColumn", Q_ARG(QVariant, QVariant("quality"))));
        QVERIFY(QMetaObject::invokeMethod(table, "setGroup", Q_ARG(QVariant, QVariant("week"))));
        QVERIFY(QMetaObject::invokeMethod(stats, "setTab", Q_ARG(QVariant, QVariant("notes"))));
        waitForBackendIdle(backend);
        QCOMPARE(lastRequest(daemon, "sessions.list").value("params").toObject().value("values").toObject().size(), 2);
        shotWindow(window, "stats-notes.png");
        QVERIFY(hasVisualText(window, "· Arabic grammar course"));
        QVERIFY(QMetaObject::invokeMethod(bar, "clearMetrics"));
        waitForBackendIdle(backend);
        flick->setProperty("contentY", 300);
        shotWindow(window, "stats-notes-unfiltered.png");
        flick->setProperty("contentY", 0);
        QVERIFY(QMetaObject::invokeMethod(stats, "setTab", Q_ARG(QVariant, QVariant("charts"))));
        waitForBackendIdle(backend);

        // A table row for an epic opens its page; the page has totals, minutes
        // per day, its tasks and its notes, and goes back to Statistics.
        QVERIFY(QMetaObject::invokeMethod(stats, "setTab", Q_ARG(QVariant, QVariant("table"))));
        QVERIFY(QMetaObject::invokeMethod(table, "setGroup", Q_ARG(QVariant, QVariant("epic"))));
        waitForBackendIdle(backend);
        QObject *openRow = nullptr;
        for (QObject *object : visualNamed(window, "epicRowOpen"))
            if (object->property("enabled").toBool()) { openRow = object; break; }
        QVERIFY(openRow);
        auto *openItem = qobject_cast<QQuickItem *>(openRow); QVERIFY(openItem);
        QTest::mouseClick(window, Qt::LeftButton, {}, openItem->mapToScene(QPointF(openItem->width() / 2, openItem->height() / 2)).toPoint());
        QCOMPARE(root->property("page").toInt(), 4);
        QCOMPARE(root->property("epicBackPage").toInt(), 2);
        QCOMPARE(root->property("epicId").toString(), QString("e-juz"));
        QTRY_VERIFY_WITH_TIMEOUT(!backend.epicSessions().isEmpty() && !backend.epicSeries().isEmpty(), 3000);
        QObject *epicPage = findByName(root.data(), "epicPage"); QVERIFY(epicPage);
        QCOMPARE(findByName(epicPage, "epicTitle")->property("text").toString(), QString("Memorise Juz 30"));
        QCOMPARE(findByName(epicPage, "epicSubtitle")->property("text").toString(), QString("Quran  ·  Active"));
        shotWindow(window, "epic-page.png");
        QObject *epicFlick = epicPage->property("contentItem").value<QObject *>(); QVERIFY(epicFlick);
        epicFlick->setProperty("contentY", epicFlick->property("contentHeight").toReal() - epicFlick->property("height").toReal());
        shotWindow(window, "epic-page-notes.png");
        epicFlick->setProperty("contentY", 0);
        QVERIFY(QMetaObject::invokeMethod(findByName(epicPage, "epicLogButton"), "clicked"));
        QTRY_VERIFY_WITH_TIMEOUT(prompt->property("visible").toBool(), 2000);
        QCOMPARE(prompt->property("epicId").toString(), QString("e-juz"));
        root->setProperty("sessionPromptVisible", false);
        QVERIFY(QMetaObject::invokeMethod(findByName(epicPage, "epicFinishButton"), "clicked"));
        QTRY_VERIFY_WITH_TIMEOUT(!lastRequest(daemon, "epic.edit").isEmpty(), 3000);
        QVERIFY(QMetaObject::invokeMethod(findByName(epicPage, "epicBackButton"), "clicked"));
        QCOMPARE(root->property("page").toInt(), 2);
        QVERIFY(QMetaObject::invokeMethod(table, "setGroup", Q_ARG(QVariant, QVariant("week"))));
        QVERIFY(QMetaObject::invokeMethod(stats, "setTab", Q_ARG(QVariant, QVariant("charts"))));

        // A timer that ran past Sleep: Worked is capped and the panel says why.
        state->status["running"] = TrackingFixture::staleRunningSession();
        root->setProperty("page", 0);
        backend.reload();
        waitForBackendIdle(backend);
        QTRY_VERIFY_WITH_TIMEOUT(backend.running().value("stale").toBool(), 3000);
        QCOMPARE(findByName(runningCard, "runningElapsed")->property("text").toString(), QString("120:00"));
        shotWindow(window, "running-card-stale.png");
        QVERIFY(QMetaObject::invokeMethod(root.data(), "pauseRunning"));
        QTRY_VERIFY_WITH_TIMEOUT(prompt->property("visible").toBool(), 2000);
        QCOMPARE(findByName(root.data(), "sessionMinutesField")->property("text").toString(), QString("120"));
        shotWindow(window, "prompt-pause-stale.png");
        root->setProperty("sessionPromptVisible", false);

        QVERIFY2(backend.error().isEmpty(), qPrintable(backend.error()));
        window->hide();
    }

    static QJsonArray resourceRowsFor(const QString &item) {
        QJsonArray rows = rpcFixture("resources_list").value("result").toArray();
        for (int i = 0; i < rows.size(); ++i) {
            QJsonObject row = rows.at(i).toObject();
            row["item_id"] = item;
            row["id"] = QString("r%1").arg(i + 1);
            // The reminder is long overdue and open; the link keeps its title.
            if (row.value("kind") == "reminder") row["remind_at"] = "2020-01-02T08:00:00Z";
            rows[i] = row;
        }
        return rows;
    }
    static void setRowResources(QJsonObject &day, const QString &id, int notes, int links, int reminders) {
        QJsonArray blocks = day.value("blocks").toArray();
        for (int b = 0; b < blocks.size(); ++b) {
            QJsonObject block = blocks.at(b).toObject();
            QJsonArray items = block.value("items").toArray();
            for (int i = 0; i < items.size(); ++i) {
                QJsonObject item = items.at(i).toObject();
                if (item.value("id").toString() == id)
                    item["resources"] = QJsonObject{{"notes", notes}, {"links", links}, {"reminders", reminders}};
                items[i] = item;
            }
            block["items"] = items;
            blocks[b] = block;
        }
        day["blocks"] = blocks;
    }
    void resourceRequestsFollowCanonicalFixtures() {
        for (const char *name : {"resource_add_link", "resource_add_reminder", "resource_edit_done"})
            QVERIFY2(Dto::decode<Dto::MutationResult>(fixture(name)), name);
        const QJsonArray listed = rpcFixture("resources_list").value("result").toArray();
        QCOMPARE(listed.size(), 3);
        for (const char *key : {"id", "item_id", "epic_id", "kind", "title", "body", "url", "remind_at", "done", "position", "version"})
            QVERIFY2(listed.first().toObject().contains(key), key);
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        auto state = trackingState();
        state->resources = resourceRowsFor("dhikr");
        daemon.setHandler(trackingHandler(state));
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        backend.reload();
        waitForBackendIdle(backend);

        backend.loadResources("item", "dhikr");
        QTRY_COMPARE_WITH_TIMEOUT(backend.resources().size(), 3, 3000);
        QJsonObject params = lastRequest(daemon, "resources.list").value("params").toObject();
        QCOMPARE(params.value("item_id").toString(), QString("dhikr"));
        QVERIFY(!params.contains("epic_id"));

        backend.addResource({{"kind", "link"}, {"url", "https://example.org/books?q=first"}, {"title", "Reading"}});
        QTRY_VERIFY_WITH_TIMEOUT(!lastRequest(daemon, "resource.add").isEmpty(), 3000);
        params = lastRequest(daemon, "resource.add").value("params").toObject();
        QCOMPARE(params.value("item_id").toString(), QString("dhikr"));
        QCOMPARE(params.value("kind").toString(), QString("link"));
        QCOMPARE(params.value("url").toString(), QString("https://example.org/books?q=first"));
        QCOMPARE(params.value("title").toString(), QString("Reading"));
        QVERIFY(params.contains("op_id") && !params.contains("body") && !params.contains("remind_at") && !params.contains("epic_id"));

        backend.addResource({{"kind", "reminder"}, {"title", "Check opening times"}, {"remind_at", "2031-05-14T04:30:00Z"}});
        QTRY_COMPARE_WITH_TIMEOUT(requestCount(daemon, "resource.add"), 2, 3000);
        params = lastRequest(daemon, "resource.add").value("params").toObject();
        QCOMPARE(params.value("kind").toString(), QString("reminder"));
        QCOMPARE(params.value("remind_at").toString(), QString("2031-05-14T04:30:00Z"));

        backend.addResource({{"kind", "note"}, {"body", "First line\nSecond line"}, {"title", "  "}});
        QTRY_COMPARE_WITH_TIMEOUT(requestCount(daemon, "resource.add"), 3, 3000);
        params = lastRequest(daemon, "resource.add").value("params").toObject();
        QCOMPARE(params.value("body").toString(), QString("First line\nSecond line"));
        QVERIFY(!params.contains("title") && !params.contains("url"));

        backend.editResource("r2", {{"done", true}});
        QTRY_VERIFY_WITH_TIMEOUT(!lastRequest(daemon, "resource.edit").isEmpty(), 3000);
        params = lastRequest(daemon, "resource.edit").value("params").toObject();
        QCOMPARE(params.value("id").toString(), QString("r2"));
        QVERIFY(params.value("done").toBool());
        QCOMPARE(params.value("expected_version").toInt(), 1);
        QVERIFY(params.contains("op_id") && !params.contains("title") && !params.contains("remind_at"));

        backend.editResource("r1", {{"title", ""}, {"url", "https://example.org/other"}});
        QTRY_COMPARE_WITH_TIMEOUT(requestCount(daemon, "resource.edit"), 2, 3000);
        params = lastRequest(daemon, "resource.edit").value("params").toObject();
        QVERIFY(params.value("title").isNull() && params.contains("title"));
        QCOMPARE(params.value("url").toString(), QString("https://example.org/other"));

        backend.removeResource("r3");
        QTRY_VERIFY_WITH_TIMEOUT(!lastRequest(daemon, "resource.remove").isEmpty(), 3000);
        params = lastRequest(daemon, "resource.remove").value("params").toObject();
        QCOMPARE(params.value("id").toString(), QString("r3"));
        QCOMPARE(params.value("expected_version").toInt(), 1);
        QTRY_VERIFY_WITH_TIMEOUT(backend.resources().size() >= 3, 3000);

        // Epic targets use epic_id and never item_id.
        backend.loadResources("epic", "e-grammar");
        QTRY_VERIFY_WITH_TIMEOUT(lastRequest(daemon, "resources.list").value("params").toObject().contains("epic_id"), 3000);
        params = lastRequest(daemon, "resources.list").value("params").toObject();
        QCOMPARE(params.value("epic_id").toString(), QString("e-grammar"));
        QVERIFY(!params.contains("item_id"));
        QTRY_COMPARE_WITH_TIMEOUT(backend.resources().size(), 0, 3000);
        backend.addResource({{"kind", "note"}, {"body", "Epic note"}});
        QTRY_COMPARE_WITH_TIMEOUT(requestCount(daemon, "resource.add"), 4, 3000);
        params = lastRequest(daemon, "resource.add").value("params").toObject();
        QCOMPARE(params.value("epic_id").toString(), QString("e-grammar"));
        QVERIFY(!params.contains("item_id"));

        // A `changed` notification for resources refreshes the open list.
        const int lists = requestCount(daemon, "resources.list");
        daemon.notifyChanged(500, {"resources"});
        QTRY_VERIFY_WITH_TIMEOUT(requestCount(daemon, "resources.list") > lists, 3000);
        backend.closeResources();
        QVERIFY(backend.resources().isEmpty());
        QVERIFY2(backend.error().isEmpty(), qPrintable(backend.error()));
    }
    void resourceWritesUseStdioRpcOffline() {
        QTemporaryDir dir; Backend b(nullptr, fake(dir)); QSignalSpy spy(&b, &Backend::requestFinished);
        b.reload(); QTRY_COMPARE(spy.size(), 4);
        b.loadResources("item", "<id:2>");
        b.addResource({{"kind", "link"}, {"url", "https://example.org"}});
        b.editResource("r1", {{"done", true}});
        b.removeResource("r1");
        QTRY_COMPARE_WITH_TIMEOUT(offlineRequests(dir).size(), 3, 3000);
        const auto requests = offlineRequests(dir);
        QCOMPARE(requests[0].value("method").toString(), QString("resource.add"));
        QCOMPARE(requests[0].value("params").toObject().value("item_id").toString(), QString("<id:2>"));
        QCOMPARE(requests[1].value("method").toString(), QString("resource.edit"));
        QVERIFY(requests[1].value("params").toObject().value("done").toBool());
        QCOMPARE(requests[2].value("method").toString(), QString("resource.remove"));
    }
    void resourceDrawerScreens() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        auto state = trackingState();
        setRowResources(state->day, "dhikr", 1, 1, 1);
        state->resources = resourceRowsFor("dhikr");
        state->backlog = backlogRows();
        daemon.setHandler(trackingHandler(state));
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        Theme theme(dir.path());
        QQmlEngine engine; engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl::fromLocalFile(QStringLiteral(TEST_SOURCE_DIR "/../src/qml/Main.qml")));
        QScopedPointer<QObject> root(component.create()); QVERIFY2(root, qPrintable(component.errorString()));
        auto *window = qobject_cast<QQuickWindow *>(root.data()); QVERIFY(window);
        window->setWidth(1440); window->setHeight(900); window->show();
        backend.reload();
        waitForBackendIdle(backend);

        // The Today row shows its count and opens the drawer.
        const int row = timelineItem(backend.dayTimeline(), "dhikr");
        QVERIFY(row >= 0);
        QCOMPARE(backend.dayTimeline()->data(backend.dayTimeline()->index(row, 0), DayTimelineModel::ResourceCountRole).toInt(), 3);
        QObject *chip = nullptr;
        for (QObject *candidate : visualNamed(window, "resourcesButton"))
            if (candidate->property("count").toInt() == 3) chip = candidate;
        QVERIFY(chip);
        QVERIFY(QMetaObject::invokeMethod(chip, "clicked"));
        QObject *drawer = findByName(root.data(), "resourceDrawer"); QVERIFY(drawer);
        QTRY_VERIFY_WITH_TIMEOUT(drawer->property("opened").toBool(), 2000);
        QTRY_COMPARE_WITH_TIMEOUT(backend.resources().size(), 3, 3000);
        QCOMPARE(lastRequest(daemon, "resources.list").value("params").toObject().value("item_id").toString(), QString("dhikr"));
        QTRY_VERIFY_WITH_TIMEOUT(!visualNamed(window, "resourceOpen_r1").isEmpty(), 2000);
        shotWindow(window, "resource-drawer.png");
        QVERIFY(!visualNamed(window, "resourceEdit_r3").isEmpty());

        // Add a note: the form takes any text; nothing is required.
        QVERIFY(QMetaObject::invokeMethod(visualNamed(window, "resourceAdd").value(0), "clicked"));
        QObject *form = findByName(root.data(), "resourceForm"); QVERIFY(form);
        QTRY_VERIFY_WITH_TIMEOUT(form->property("visible").toBool(), 2000);
        QVERIFY(visualNamed(window, "resourceSave").value(0)->property("enabled").toBool());
        shotWindow(window, "resource-add-form.png");
        QVERIFY(QMetaObject::invokeMethod(visualNamed(window, "resourceKind_link").value(0), "clicked"));
        QObject *save = visualNamed(window, "resourceSave").value(0);
        QVERIFY(!save->property("enabled").toBool());  // a link needs a valid URL
        visualNamed(window, "resourceUrl").value(0)->setProperty("text", "https://example.org/read");
        QTRY_VERIFY(save->property("enabled").toBool());
        shotWindow(window, "resource-add-link.png");
        QVERIFY(QMetaObject::invokeMethod(visualNamed(window, "resourceKind_reminder").value(0), "clicked"));
        QVERIFY(save->property("enabled").toBool());  // the default is tomorrow 09:00
        shotWindow(window, "resource-add-reminder.png");
        visualNamed(window, "resourceDate").value(0)->setProperty("text", "2031-13-45");
        QTRY_VERIFY(!save->property("enabled").toBool());
        QVERIFY(QMetaObject::invokeMethod(visualNamed(window, "resourceTomorrow").value(0), "clicked"));
        QVERIFY(save->property("enabled").toBool());
        visualNamed(window, "resourceTitle").value(0)->setProperty("text", "Ring the shop");
        QVERIFY(QMetaObject::invokeMethod(save, "clicked"));
        QTRY_VERIFY_WITH_TIMEOUT(!lastRequest(daemon, "resource.add").isEmpty(), 3000);
        QJsonObject params = lastRequest(daemon, "resource.add").value("params").toObject();
        QCOMPARE(params.value("kind").toString(), QString("reminder"));
        QCOMPARE(params.value("title").toString(), QString("Ring the shop"));
        QVERIFY(QDateTime::fromString(params.value("remind_at").toString(), Qt::ISODate).isValid());
        QVERIFY(params.value("remind_at").toString().endsWith("Z"));
        QTRY_VERIFY_WITH_TIMEOUT(!form->property("visible").toBool(), 3000);

        // Tick the overdue reminder off.
        QQuickItem *done = qobject_cast<QQuickItem *>(visualNamed(window, "resourceDone_r2").value(0)); QVERIFY(done);
        QTest::mouseClick(window, Qt::LeftButton, {}, done->mapToScene({done->width() / 2, done->height() / 2}).toPoint());
        QTRY_VERIFY_WITH_TIMEOUT(!lastRequest(daemon, "resource.edit").isEmpty(), 3000);
        QVERIFY(lastRequest(daemon, "resource.edit").value("params").toObject().value("done").toBool());
        // Edit the link.
        QVERIFY(QMetaObject::invokeMethod(visualNamed(window, "resourceEdit_r1").value(0), "clicked"));
        QTRY_VERIFY_WITH_TIMEOUT(form->property("visible").toBool(), 2000);
        QCOMPARE(visualNamed(window, "resourceUrl").value(0)->property("text").toString(), QString("https://example.org/books?q=first"));
        shotWindow(window, "resource-edit-link.png");
        QVERIFY(QMetaObject::invokeMethod(visualNamed(window, "resourceCancel").value(0), "clicked"));
        QVERIFY(QMetaObject::invokeMethod(visualNamed(window, "resourceRemove_r3").value(0), "clicked"));
        QTRY_VERIFY_WITH_TIMEOUT(!lastRequest(daemon, "resource.remove").isEmpty(), 3000);
        QCOMPARE(lastRequest(daemon, "resource.remove").value("params").toObject().value("id").toString(), QString("r3"));
        QVERIFY(QMetaObject::invokeMethod(visualNamed(window, "resourceClose").value(0), "clicked"));
        QTRY_VERIFY_WITH_TIMEOUT(!drawer->property("opened").toBool(), 2000);

        // The Backlog page opens the same drawer for an item.
        root->setProperty("page", 5);
        QTRY_COMPARE_WITH_TIMEOUT(backend.backlogItems().size(), 5, 3000);
        QObject *backlogChip = visualNamed(window, "backlogResources_b1").value(0); QVERIFY(backlogChip);
        QVERIFY(QMetaObject::invokeMethod(backlogChip, "clicked"));
        QTRY_VERIFY_WITH_TIMEOUT(drawer->property("opened").toBool(), 2000);
        QTRY_VERIFY_WITH_TIMEOUT(lastRequest(daemon, "resources.list").value("params").toObject().value("item_id").toString() == "b1", 3000);
        shotWindow(window, "resource-drawer-backlog.png");
        QVERIFY(QMetaObject::invokeMethod(visualNamed(window, "resourceClose").value(0), "clicked"));
        QVERIFY2(backend.error().isEmpty(), qPrintable(backend.error()));
        window->hide();
    }

    // goals_list.json with its placeholder ids mapped onto the tracking
    // fixture's rows: g1 all work, g2 Quran, g3 the Juz epic, g4 the dhikr
    // task, g5 a Focus goal and g6 a Distractions limit.
    static QJsonArray goalRows() {
        QByteArray text = QJsonDocument(rpcFixture("goals_list").value("result").toArray()).toJson(QJsonDocument::Compact);
        const QList<QPair<QByteArray, QByteArray>> ids{
            {"<id:1>", "g1"}, {"<id:2>", "g2"}, {"<id:3>", "quran"}, {"<id:4>", "e-juz"}, {"<id:5>", "g3"},
            {"<id:6>", "g4"}, {"<id:7>", "dhikr"}, {"<id:8>", "g5"}, {"<id:9>", "g6"}};
        for (const auto &id : ids) text.replace(id.first, id.second);
        return QJsonDocument::fromJson(text).array();
    }
    static std::shared_ptr<TrackingState> goalsState() {
        auto state = trackingState();
        state->goals = goalRows();
        state->goalHistory = rpcFixture("goals_history").value("result").toArray();
        return state;
    }
    void insightsScreensAndSessionEditor() {
        QTemporaryDir dir;
        ReportsPreviewBackend backend(fake(dir), daemon_path(dir), backlogRows(), backlogRows());
        const QJsonObject report = fixture("insights");
        backend.report = report.toVariantMap();
        backend.goalRows = goalRows().toVariantList();
        const auto successor = fixture("goal_edit_successor").value("changed").toObject().value("goals").toArray().first().toObject();
        backend.goalRows = {backend.goalRows.at(1), successor.toVariantMap()};
        backend.historyId = successor.value("id").toString();
        backend.history = rpcFixture("goals_history").value("result").toArray().toVariantList();
        Theme theme(dir.path());
        QQmlEngine engine;
        engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl("qrc:/qml/Main.qml"));
        QScopedPointer<QObject> root(component.create());
        QVERIFY2(root, qPrintable(component.errorString()));
        auto *window = qobject_cast<QQuickWindow *>(root.data());
        QVERIFY(window);
        window->resize(1280, 880);
        window->show();
        auto *link = qobject_cast<QQuickItem *>(findByName(window, "insightsSidebarItem"));
        QVERIFY(link);
        QTest::mouseClick(window, Qt::LeftButton, {}, link->mapToScene(QPointF(link->width()/2, link->height()/2)).toPoint());
        QCOMPARE(root->property("page").toInt(), 7);
        auto click = [&](const QString &name) {
            QObject *item = firstShown(window, name);
            return item && QMetaObject::invokeMethod(item, "clicked");
        };
        auto *page = firstShown(window, "insightsPage");
        QVERIFY(page);
        auto *bar = page->property("filterBar").value<QQuickItem *>();
        QVERIFY(bar);
        QVERIFY(QMetaObject::invokeMethod(bar, "setCustom", Q_ARG(QVariant, QVariant("2031-05-11")),
                                         Q_ARG(QVariant, QVariant("2031-05-14"))));
        QVERIFY(firstShown(window, "insightGroupRow_sleep"));
        QVERIFY(hasVisualText(window, "3.4"));
        QVERIFY(hasVisualText(window, "2.4"));
        QVERIFY(hasVisualText(window, "1 of 2 weeks with values"));
        QVERIFY(hasVisualText(window, "Distractions/h"));
        QVERIFY(hasVisualText(window, "Avg focus"));
        QVERIFY(hasVisualText(window, "Sessions"));
        shotWindow(window, "insights-blocks.png");
        QVERIFY(click("insightsFilters"));
        QVERIFY(firstShown(window, "statsTaskSearch"));
        QVERIFY(!firstShown(window, "metricAddChip"));
        window->resize(960, 1280);
        shotWindow(window, "insights-filters.png");
        auto *endDate = qobject_cast<QQuickItem *>(firstShown(window, "filterEndDate"));
        QVERIFY(endDate);
        QVERIFY(endDate->mapToItem(bar, QPointF(endDate->width(), 0)).x() <= bar->width());
        QVERIFY(bar->width() <= page->property("availableWidth").toReal());
        window->resize(1280, 880);
        QVERIFY(click("insightsFilters"));
        QVERIFY(click("insightGroup_hour"));
        QVERIFY(hasVisualText(window, "Timed sessions, grouped by local start time"));
        window->resize(1280, 1180);
        shotWindow(window, "insights-hours.png");
        window->resize(1280, 880);
        {
            auto *flick = page->property("contentItem").value<QQuickItem *>();
            QVERIFY(flick);
            flick->setProperty("contentY", flick->property("contentHeight").toReal());
            shotWindow(window, "insights-scrolled.png");
            flick->setProperty("contentY", 0);
        }
        QVERIFY(click("insightGroup_weekday"));
        QVERIFY(hasVisualText(window, "Monday"));
        shotWindow(window, "insights-weekdays.png");
        auto *weekly = firstShown(window, "insightsWeekly");
        QVERIFY(weekly);
        QCOMPARE(weekly->property("points").toList().size(), report.value("distractions_by_week").toArray().size());
        QVERIFY(QMetaObject::invokeMethod(findItemByName(weekly, "chartTableButton"), "clicked"));
        QVERIFY(hasVisualText(window, "WEEK OF"));
        const auto weeklyRows = weekly->property("tableRows").toList();
        QCOMPARE(weeklyRows.size(), 2);
        QCOMPARE(weeklyRows.first().toList().last().toString(), QString("3"));
        QCOMPARE(weeklyRows.last().toList().last().toString(), QString("—"));
        shotWindow(window, "insights-weekly-table.png");
        QVERIFY(click("insightsTab_plan"));
        QVERIFY(hasVisualText(window, "Unplanned"));
        const auto plans = report.value("on_plan").toArray();
        for (const auto &entry : plans) {
            const auto row = entry.toObject();
            auto *rate = firstShown(window, "onPlanRate_" + row.value("item_id").toString());
            QVERIFY(rate);
            if (!row.value("planned_block").isNull()) {
                const double percent = row.value("on_plan_minutes").toDouble() * 100 / row.value("minutes").toDouble();
                QCOMPARE(rate->property("text").toString(), QString::number(qRound(percent * 10) / 10.) + "%");
            }
        }
        QCOMPARE(firstShown(window, "matrix_fajr_dhuhr")->property("minutes").toInt(), 15);
        QCOMPARE(firstShown(window, "matrix_unplanned_dhuhr")->property("minutes").toInt(), 10);
        window->resize(960, 640);
        shotWindow(window, "insights-plan.png");
        window->resize(1280, 880);
        QVERIFY(click("insightsTab_sessions"));
        QVERIFY(hasVisualText(window, "Chapter 1"));
        for (const QString key : {"best", "hardest"}) {
            const auto ranking = report.value(key).toArray();
            QVERIFY(!ranking.isEmpty());
            const auto session = ranking.first().toObject();
            auto *row = firstShown(window, "insightSession_" + key + "_" + session.value("id").toString());
            QVERIFY(row);
            QVERIFY(QMetaObject::invokeMethod(row, "open", Q_ARG(QVariant, session.toVariantMap())));
            auto *prompt = findByName(window, "sessionPrompt");
            QVERIFY(prompt && window->property("sessionPromptVisible").toBool());
            QCOMPARE(prompt->property("sessionId").toString(), session.value("id").toString());
            QCOMPARE(prompt->property("editVersion").toInt(), session.value("version").toInt());
            QCOMPARE(prompt->property("notes").toList(), session.value("notes").toArray().toVariantList());
            if (key == "best") shotWindow(window, "insights-session-editor.png");
            window->setProperty("sessionPromptVisible", false);
        }
        shotWindow(window, "insights-sessions.png");
        backend.report["best"] = QVariantList{};
        backend.report["hardest"] = QVariantList{};
        emit backend.previewChanged();
        QVERIFY(hasVisualText(window, "Best sessions  ·  0 sessions"));
        window->setProperty("page", 6);
        QVERIFY(hasVisualText(window, "Study"));
        QVERIFY(hasVisualText(window, "Met in 1 of 2 periods"));
        shotWindow(window, "goals-stored-scope-history.png");
    }

    void insightsDateRangeAndScope() {
        QTemporaryDir dir;
        FakeDaemon daemon(daemon_path(dir));
        if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        const auto report = fixture("insights");
        auto tracking = trackingHandler(trackingState());
        daemon.setHandler([&](const QJsonObject &request, QLocalSocket *socket, FakeDaemon *server) {
            if (request.value("method") == "insights.get") server->respond(socket, request, report);
            else tracking(request, socket, server);
        });
        Backend backend(nullptr, fake(dir), 3000, daemon_path(dir));
        backend.reload();
        waitForBackendIdle(backend);
        backend.loadInsights({{"days", 7}});
        QTRY_COMPARE(backend.insights(), report.toVariantMap());
        auto params = lastRequest(daemon, "insights.get").value("params").toObject();
        const QDate today = QDate::fromString(backend.todayDay(), Qt::ISODate);
        QCOMPARE(params, (QJsonObject{{"from", today.addDays(-6).toString(Qt::ISODate)}, {"to", backend.todayDay()}}));
        const QJsonObject scoped{{"from", "2031-05-01"}, {"to", "2031-05-14"}, {"tag_id", "none"},
                                 {"project_id", "none"}, {"epic_id", "none"}, {"item_id", "read"}};
        backend.loadInsights(scoped.toVariantMap());
        QTRY_COMPARE(lastRequest(daemon, "insights.get").value("params").toObject(), scoped);
        backend.loadInsights({{"from", "2031-05-14"}, {"to", "2031-05-01"}});
        QVERIFY(backend.insights().isEmpty());
        QCOMPARE(requestCount(daemon, "insights.get"), 2);
    }

    void launchPageArgument() {
        QString warning;
        QCOMPARE(launch::startPage({"salah-app"}, &warning), QString("today"));
        QVERIFY(warning.isEmpty());
        for (const QString &name : launch::pageNames()) {
            QCOMPARE(launch::startPage({"salah-app", "--page", name}, &warning), name);
            QCOMPARE(launch::startPage({"salah-app", "--page=" + name}, &warning), name);
            QVERIFY(warning.isEmpty());
        }
        QCOMPARE(launch::startPage({"salah-app", "--page", "Review"}, &warning), QString("review"));
        QCOMPARE(launch::startPage({"salah-app", "--page", "nowhere"}, &warning), QString("today"));
        QVERIFY(warning.contains("nowhere"));
        warning.clear();
        QCOMPARE(launch::startPage({"salah-app", "--page"}, &warning), QString("today"));
        QVERIFY(!warning.isEmpty());

        QTemporaryDir dir;
        TrackingUiPreviewBackend backend(fake(dir), daemon_path(dir), backlogRows(), backlogRows());
        Theme theme(dir.path());
        QQmlEngine engine;
        engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl("qrc:/qml/Main.qml"));
        QScopedPointer<QObject> root(component.create());
        QVERIFY2(root, qPrintable(component.errorString()));
        const QList<QPair<QString, int>> pages{{"today", 0}, {"routines", 1}, {"statistics", 2}, {"settings", 3},
                                               {"backlog", 5}, {"goals", 6}, {"insights", 7}, {"review", 8}};
        for (const auto &entry : pages) {
            QVariant ok;
            QVERIFY(QMetaObject::invokeMethod(root.data(), "showPage", Q_RETURN_ARG(QVariant, ok), Q_ARG(QVariant, entry.first)));
            QVERIFY(ok.toBool());
            QCOMPARE(root->property("page").toInt(), entry.second);
        }
        QVariant ok;
        QVERIFY(QMetaObject::invokeMethod(root.data(), "showPage", Q_RETURN_ARG(QVariant, ok), Q_ARG(QVariant, "nowhere")));
        QVERIFY(!ok.toBool());
    }

    void reviewPageScreens() {
        QTemporaryDir dir;
        ReportsPreviewBackend backend(fake(dir), daemon_path(dir), backlogRows(), backlogRows());
        const QJsonObject review = fixture("review_get");
        backend.reviewData = review.toVariantMap();
        Theme theme(dir.path());
        QQmlEngine engine;
        engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl("qrc:/qml/Main.qml"));
        QScopedPointer<QObject> root(component.create());
        QVERIFY2(root, qPrintable(component.errorString()));
        auto *window = qobject_cast<QQuickWindow *>(root.data());
        QVERIFY(window);
        window->resize(1280, 1500);
        window->show();
        auto *link = qobject_cast<QQuickItem *>(findByName(window, "reviewSidebarItem"));
        QVERIFY(link);
        QTest::mouseClick(window, Qt::LeftButton, {}, link->mapToScene(QPointF(link->width()/2, link->height()/2)).toPoint());
        QCOMPARE(root->property("page").toInt(), 8);
        auto *page = firstShown(window, "reviewPage");
        QVERIFY(page);
        // Default: the last full week, a Monday, one before the current week.
        backend.date = "2031-05-26";
        emit backend.todayDayChanged();
        QTRY_COMPARE(backend.reads.last(), QString("2031-05-19"));
        backend.date = "2031-05-19";
        emit backend.todayDayChanged();
        QTRY_COMPARE(backend.reads.last(), QString("2031-05-12"));
        const QString current = page->property("currentWeek").toString();
        const QDate last = QDate::fromString(page->property("week").toString(), Qt::ISODate);
        QVERIFY(last.isValid());
        QCOMPARE(last.dayOfWeek(), 1);
        QCOMPARE(last.addDays(7).toString(Qt::ISODate), current);
        QVERIFY(!firstShown(window, "reviewNextWeek")->property("enabled").toBool() == false);
        QVERIFY(QMetaObject::invokeMethod(firstShown(window, "reviewNextWeek"), "clicked"));
        QCOMPARE(page->property("week").toString(), current);
        QVERIFY(!firstShown(window, "reviewNextWeek")->property("enabled").toBool());
        QVERIFY(QMetaObject::invokeMethod(firstShown(window, "reviewNextWeek"), "clicked"));
        QCOMPARE(page->property("week").toString(), current);
        QVERIFY(QMetaObject::invokeMethod(firstShown(window, "reviewPrevWeek"), "clicked"));
        QCOMPARE(page->property("week").toString(), last.toString(Qt::ISODate));
        QCOMPARE(page->property("week").toString(), review.value("week").toString());
        emit backend.previewChanged();
        QVERIFY(hasVisualText(window, "Minutes by project"));
        QVERIFY(hasVisualText(window, "No project"));
        QVERIFY(hasVisualText(window, "No epic"));
        QVERIFY(hasVisualText(window, "Study"));
        QVERIFY(hasVisualText(window, "Chapter"));
        QVERIFY(hasVisualText(window, "Goals met  ·  3"));
        QVERIFY(hasVisualText(window, "Sessions with notes  ·  2"));
        QCOMPARE(firstShown(window, "reviewBacklogCount")->property("text").toString(), QString("0"));
        QCOMPARE(firstShown(window, "reviewDistractions")->property("text").toString(), QString::number(review.value("summary").toObject().value("distractions_per_hour").toDouble()));
        QCOMPARE(firstShown(window, "reviewPreviousDistractions")->property("text").toString(), QString("6"));
        QCOMPARE(firstShown(window, "reviewNote")->property("text").toString(), QString("Keep reading.\nTry mornings."));
        QVERIFY(firstShown(window, "reviewSession_" + review.value("summary").toObject().value("sessions_with_notes").toArray().first().toObject().value("id").toString()));
        QVERIFY(!firstShown(window, "reviewSave")->property("enabled").toBool());
        firstShown(window, "reviewNote")->setProperty("text", "Keep reading.\nTry mornings.\nMore.");
        QVERIFY(firstShown(window, "reviewSave")->property("enabled").toBool());
        shotWindow(window, "review-page.png");
        auto *note = firstShown(window, "reviewNote");
        auto *search = firstShown(window, "reviewNotesSearch");
        auto *save = firstShown(window, "reviewSave");
        auto *confirmation = findByName(window, "reviewSaved");
        QVERIFY(note && search && confirmation);
        const QString week = review.value("week").toString();
        search->setProperty("text", " SHOP ");
        QVERIFY(hasVisualText(window, "Sessions with notes  ·  1 of 2 shown"));
        QCOMPARE(page->property("matchingNotes").toList().first().toMap().value("id"),
                 review.value("summary").toObject().value("sessions_with_notes").toArray().first().toObject().value("id").toVariant());
        search->setProperty("text", "mOnDaY");
        QCOMPARE(page->property("matchingNotes").toList().size(), 1);
        search->setProperty("text", "missing");
        QVERIFY(hasVisualText(window, "No matching notes."));
        search->setProperty("text", "shop");
        note->setProperty("text", "My draft");
        backend.reviewData["note"] = "Changed from another client";
        emit backend.previewChanged();
        QCOMPARE(note->property("text").toString(), QString("My draft"));
        shotWindow(window, "review-draft-filter.png");
        QVERIFY(QMetaObject::invokeMethod(save, "clicked"));
        QCOMPARE(backend.submittedWeek, week);
        QCOMPARE(backend.submittedNote, QString("My draft"));
        QVERIFY(!save->property("enabled").toBool());
        note->setProperty("text", "Edited during save");
        emit backend.reviewSaveFinished(true, {}, week, "My draft");
        QVERIFY(save->property("enabled").toBool());
        QCOMPARE(confirmation->property("opacity").toReal(), 0.0);
        backend.reviewData["note"] = "My draft";
        emit backend.previewChanged();
        QCOMPARE(note->property("text").toString(), QString("Edited during save"));
        QVERIFY(QMetaObject::invokeMethod(save, "clicked"));
        QVERIFY(QMetaObject::invokeMethod(firstShown(window, "reviewPrevWeek"), "clicked"));
        const QString otherWeek = page->property("week").toString();
        backend.reviewData["week"] = otherWeek;
        backend.reviewData["note"] = "Older week";
        emit backend.previewChanged();
        QCOMPARE(search->property("text").toString(), QString());
        QCOMPARE(note->property("text").toString(), QString("Older week"));
        note->setProperty("text", "Second draft");
        emit backend.reviewSaveFinished(true, {}, week, "Edited during save");
        QCOMPARE(confirmation->property("opacity").toReal(), 0.0);
        backend.date = "2031-06-02";
        emit backend.todayDayChanged();
        QTRY_COMPARE(backend.reads.last(), otherWeek);
        QCOMPARE(page->property("currentWeek").toString(), backend.date);
        QCOMPARE(page->property("week").toString(), otherWeek);
        QVERIFY(QMetaObject::invokeMethod(firstShown(window, "reviewNextWeek"), "clicked"));
        backend.reviewData["week"] = week;
        backend.reviewData["note"] = "Edited during save";
        emit backend.previewChanged();
        QCOMPARE(note->property("text").toString(), QString("Edited during save"));
        QVERIFY(!save->property("enabled").toBool());
        QVERIFY(QMetaObject::invokeMethod(firstShown(window, "reviewPrevWeek"), "clicked"));
        backend.reviewData["week"] = otherWeek;
        backend.reviewData["note"] = "Older week";
        emit backend.previewChanged();
        QCOMPARE(note->property("text").toString(), QString("Second draft"));
        QVERIFY(QMetaObject::invokeMethod(save, "clicked"));
        emit backend.reviewSaveFinished(false, "Try again", otherWeek, "Second draft");
        QVERIFY(save->property("enabled").toBool());
        QVERIFY(QMetaObject::invokeMethod(save, "clicked"));
        emit backend.reviewSaveFinished(true, {}, otherWeek, "Second draft");
        QTRY_VERIFY(confirmation->property("opacity").toReal() > 0);
        QVERIFY(!save->property("enabled").toBool());
        auto preview = backend.reviewData.value("summary").toMap();
        preview["sessions_with_notes_count"] = 48;
        preview["sessions_with_notes_more"] = true;
        backend.reviewData["summary"] = preview;
        emit backend.previewChanged();
        QVERIFY(hasVisualText(window, "Sessions with notes  ·  2 of 48"));
        QCOMPARE(search->property("placeholderText").toString(), QString("Search notes shown here"));
        // A week with nothing: counts are zeros, never hidden.
        QVariantMap empty = backend.reviewData;
        QVariantMap summary = empty.value("summary").toMap();
        summary["by_project"] = QVariantList{};
        summary["by_epic"] = QVariantList{};
        summary["goals_met"] = QVariantList{};
        summary["sessions_with_notes"] = QVariantList{};
        summary["sessions_with_notes_count"] = 0;
        summary["sessions_with_notes_more"] = false;
        summary["distractions_per_hour"] = QVariant();
        summary["previous_distractions_per_hour"] = QVariant();
        empty["summary"] = summary;
        empty["note"] = QVariant();
        backend.reviewData = empty;
        emit backend.previewChanged();
        QVERIFY(hasVisualText(window, "Goals met  ·  0"));
        QVERIFY(hasVisualText(window, "No tracked minutes this week."));
        QVERIFY(hasVisualText(window, "No sessions with notes this week."));
        QCOMPARE(firstShown(window, "reviewDistractions")->property("text").toString(), QString("—"));
        shotWindow(window, "review-empty.png");
    }

    void reviewRequestsAndSaves() {
        QTemporaryDir dir;
        FakeDaemon daemon(daemon_path(dir));
        if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        const auto review = fixture("review_get");
        const auto saved = rpcFixture("review_save").value("result").toObject();
        auto tracking = trackingHandler(trackingState());
        daemon.setHandler([&](const QJsonObject &request, QLocalSocket *socket, FakeDaemon *server) {
            const QString method = request.value("method").toString();
            if (method == "review.get") server->respond(socket, request, review);
            else if (method == "review.save") server->respond(socket, request, saved);
            else tracking(request, socket, server);
        });
        Backend backend(nullptr, fake(dir), 3000, daemon_path(dir));
        backend.reload();
        waitForBackendIdle(backend);
        QCOMPARE(backend.reviewWeekStart("2031-05-14"), QString("2031-05-12"));
        QCOMPARE(backend.reviewWeekStart("2031-05-18", -1), QString("2031-05-05"));
        // A mid-week day is normalised to its Monday.
        backend.loadReview("2031-05-14");
        QTRY_COMPARE(backend.review(), review.toVariantMap());
        QCOMPARE(lastRequest(daemon, "review.get").value("params").toObject(), (QJsonObject{{"week", "2031-05-12"}}));
        backend.saveReview("2031-05-12", "Keep reading.\nTry mornings.");
        QTRY_COMPARE(requestCount(daemon, "review.save"), 1);
        auto params = lastRequest(daemon, "review.save").value("params").toObject();
        QCOMPARE(params.value("week").toString(), QString("2031-05-12"));
        QCOMPARE(params.value("note").toString(), QString("Keep reading.\nTry mornings."));
        QVERIFY(!params.value("op_id").toString().isEmpty());
        // An empty note removes the reflection.
        backend.saveReview("2031-05-12", "  \n ");
        QTRY_COMPARE(requestCount(daemon, "review.save"), 2);
        QCOMPARE(lastRequest(daemon, "review.save").value("params").toObject().value("note").toString(), QString());
        // Another surface saving the reflection reloads the open week.
        const int before = requestCount(daemon, "review.get");
        QSignalSpy spy(&backend, &Backend::reviewsInvalidated);
        daemon.notifyChanged(500, {"reviews"});
        QTRY_VERIFY_WITH_TIMEOUT(spy.count() >= 1, 3000);
        daemon.notifyChanged(501, {"projects"});
        QTRY_COMPARE(spy.count(), 2);
        daemon.notifyChanged(502, {"epics"});
        QTRY_COMPARE(spy.count(), 3);
        Q_UNUSED(before);
    }

    void goalFixturesDecode() {
        for (const char *name : {"goal_add", "goal_edit_successor"})
            QVERIFY2(Dto::decode<Dto::MutationResult>(fixture(name)), name);
        const QJsonArray listed = rpcFixture("goals_list").value("result").toArray();
        QCOMPARE(listed.size(), 6);
        for (const char *key : {"id", "project_id", "epic_id", "item_id", "period", "metric_id", "direction", "target",
                                "starts_on", "ends_on", "replaces_id", "position", "progress", "version"})
            QVERIFY2(listed.first().toObject().contains(key), key);
        QCOMPARE(goalRows().size(), 6);
        QCOMPARE(goalRows().at(1).toObject().value("project_id").toString(), QString("quran"));
        QCOMPARE(rpcFixture("goals_history").value("result").toArray().size(), 2);
        QCOMPARE(rpcFixture("status_goals").value("result").toObject().value("goals").toArray().size(), 6);
    }
    void goalRequestsFollowCanonicalFixtures() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        auto state = goalsState();
        daemon.setHandler(trackingHandler(state));
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy finished(&backend, &Backend::goalWriteFinished);
        backend.reload();
        waitForBackendIdle(backend);
        backend.loadGoals();
        QTRY_COMPARE_WITH_TIMEOUT(backend.goals().size(), 6, 3000);
        QVERIFY(lastRequest(daemon, "goals.list").value("params").toObject().isEmpty());

        const int listsBefore = requestCount(daemon, "goals.list");
        // Add: a minutes goal for a project sends no metric or direction.
        backend.addGoal({{"period", "day"}, {"target", 45}, {"project_id", "quran"}});
        QTRY_VERIFY_WITH_TIMEOUT(!lastRequest(daemon, "goal.add").isEmpty(), 3000);
        QJsonObject params = lastRequest(daemon, "goal.add").value("params").toObject();
        QCOMPARE(params.value("period").toString(), QString("day"));
        QCOMPARE(params.value("target").toDouble(), 45.0);
        QCOMPARE(params.value("project_id").toString(), QString("quran"));
        QVERIFY(params.contains("op_id") && !params.contains("metric_id") && !params.contains("direction")
                && !params.contains("epic_id") && !params.contains("item_id"));
        // A metric goal carries its direction; all work has no scope id.
        backend.addGoal({{"period", "week"}, {"target", 3.5}, {"metric_id", "distractions"}, {"direction", "at_most"}});
        QTRY_COMPARE_WITH_TIMEOUT(requestCount(daemon, "goal.add"), 2, 3000);
        params = lastRequest(daemon, "goal.add").value("params").toObject();
        QCOMPARE(params.value("metric_id").toString(), QString("distractions"));
        QCOMPARE(params.value("direction").toString(), QString("at_most"));
        QCOMPARE(params.value("target").toDouble(), 3.5);
        QVERIFY(!params.contains("project_id") && !params.contains("epic_id") && !params.contains("item_id"));
        backend.addGoal({{"period", "day"}, {"target", 20}, {"epic_id", "e-juz"}});
        backend.addGoal({{"period", "day"}, {"target", 10}, {"item_id", "dhikr"}});
        QTRY_COMPARE_WITH_TIMEOUT(requestCount(daemon, "goal.add"), 4, 3000);
        QCOMPARE(lastRequest(daemon, "goal.add").value("params").toObject().value("item_id").toString(), QString("dhikr"));
        // A target of zero never reaches the daemon.
        backend.addGoal({{"period", "day"}, {"target", 0}});
        QCOMPARE(requestCount(daemon, "goal.add"), 4);

        // Edit and end send the row's version; remove too.
        backend.editGoal("g1", 75);
        QTRY_VERIFY_WITH_TIMEOUT(!lastRequest(daemon, "goal.edit").isEmpty(), 3000);
        params = lastRequest(daemon, "goal.edit").value("params").toObject();
        QCOMPARE(params.value("id").toString(), QString("g1"));
        QCOMPARE(params.value("target").toDouble(), 75.0);
        QCOMPARE(params.value("expected_version").toInt(), 1);
        QVERIFY(params.contains("op_id"));
        backend.endGoal("g5");
        QTRY_VERIFY_WITH_TIMEOUT(!lastRequest(daemon, "goal.end").isEmpty(), 3000);
        params = lastRequest(daemon, "goal.end").value("params").toObject();
        QCOMPARE(params.value("id").toString(), QString("g5"));
        QCOMPARE(params.value("expected_version").toInt(), 1);
        backend.removeGoal("g6");
        QTRY_VERIFY_WITH_TIMEOUT(!lastRequest(daemon, "goal.remove").isEmpty(), 3000);
        QCOMPARE(lastRequest(daemon, "goal.remove").value("params").toObject().value("id").toString(), QString("g6"));
        QTRY_COMPARE_WITH_TIMEOUT(finished.size(), 7, 3000);
        QVERIFY(finished.at(0).at(0).toBool());
        // Every write reloads the list.
        QTRY_VERIFY_WITH_TIMEOUT(requestCount(daemon, "goals.list") > listsBefore, 3000);

        // History: the chain of any goal, ending today, at most 730 days.
        backend.loadGoalHistory("g1-successor");
        QTRY_COMPARE_WITH_TIMEOUT(backend.goalHistory().size(), 2, 3000);
        params = lastRequest(daemon, "goals.history").value("params").toObject();
        QCOMPARE(params.value("id").toString(), QString("g1-successor"));
        const QDate from = QDate::fromString(params.value("from").toString(), Qt::ISODate);
        const QDate to = QDate::fromString(params.value("to").toString(), Qt::ISODate);
        QVERIFY(from.isValid() && to.isValid() && from.daysTo(to) < 730 && from.daysTo(to) > 700);
        QCOMPARE(backend.goalHistoryId(), QString("g1-successor"));
        QCOMPARE(backend.goalHistory().first().toMap().value("target").toDouble(), 50.0);
        backend.loadGoalHistory("");
        QVERIFY(backend.goalHistory().isEmpty());

        // A `changed` notification with the goals scope reloads the list.
        const int lists = requestCount(daemon, "goals.list");
        state->goals = QJsonArray{state->goals.first()};
        daemon.notifyChanged(600, {"goals"});
        QTRY_COMPARE_WITH_TIMEOUT(backend.goals().size(), 1, 3000);
        QVERIFY(requestCount(daemon, "goals.list") > lists);
        QVERIFY2(backend.error().isEmpty(), qPrintable(backend.error()));
    }
    void goalWritesUseStdioRpcOffline() {
        QTemporaryDir dir; Backend b(nullptr, fake(dir)); QSignalSpy spy(&b, &Backend::requestFinished);
        b.reload(); QTRY_COMPARE(spy.size(), 4);
        b.addGoal({{"period", "week"}, {"target", 120}});
        b.editGoal("g1", 90);
        b.endGoal("g1");
        QTRY_COMPARE_WITH_TIMEOUT(offlineRequests(dir).size(), 3, 3000);
        const auto requests = offlineRequests(dir);
        QCOMPARE(requests[0].value("method").toString(), QString("goal.add"));
        QCOMPARE(requests[1].value("method").toString(), QString("goal.edit"));
        QCOMPARE(requests[2].value("method").toString(), QString("goal.end"));
    }
    void goalEditorsAndHistoryInCliFallback() {
        QTemporaryDir dir;
        fake(dir);
        const QString program = dir.filePath("goals-cli");
        write(program, R"SH(#!/bin/sh
cd "$(dirname "$0")"
case "$1 $2" in
  "goal list") cat goals.json ;;
  "goal history") cat history.json ;;
  "item list") printf '[]\n' ;;
  *) exec ./salah "$@" ;;
esac
)SH");
        QFile::setPermissions(program, QFile::ReadOwner | QFile::WriteOwner | QFile::ExeOwner);
        auto goal = goalRows().at(4).toObject();
        goal["target"] = 4.25;
        goal["starts_on"] = goal.value("progress").toObject().value("from");
        goal["replaces_id"] = "previous";
        write(dir.filePath("goals.json"), QJsonDocument(QJsonArray{goal}).toJson());
        write(dir.filePath("history.json"), QJsonDocument(rpcFixture("goals_history").value("result").toArray()).toJson());

        Backend backend(nullptr, program, 3000, daemon_path(dir));
        backend.reload();
        waitForBackendIdle(backend);
        QVERIFY(backend.offlineMode());
        Theme theme(dir.path());
        QQmlEngine engine;
        engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl("qrc:/qml/Main.qml"));
        QScopedPointer<QObject> root(component.create());
        QVERIFY2(root, qPrintable(component.errorString()));
        auto *window = qobject_cast<QQuickWindow *>(root.data());
        QVERIFY(window);
        window->resize(1280, 1040);
        window->show();
        window->setProperty("page", 6);
        QTRY_COMPARE(backend.goals().size(), 1);
        QTRY_VERIFY(firstShown(window, "goalRow_g5"));
        QVERIFY(firstShown(window, "goalRemoveButton"));
        QVERIFY(QMetaObject::invokeMethod(firstShown(window, "goalEditButton"), "clicked"));
        QCOMPARE(firstShown(window, "goalTargetField")->property("text").toString(), QString("4.25"));
        shotWindow(window, "goals-full-target.png");
        QVERIFY(QMetaObject::invokeMethod(firstShown(window, "goalEditSave"), "clicked"));
        waitForBackendIdle(backend);
        QVERIFY(offlineRequests(dir).isEmpty());

        QVERIFY(QMetaObject::invokeMethod(firstShown(window, "goalHistoryButton"), "clicked"));
        QTRY_COMPARE(backend.goalHistory().size(), 2);
        // The removed intermediate version is missing from goals.list.
        goal["id"] = "latest";
        goal["replaces_id"] = "intermediate";
        write(dir.filePath("goals.json"), QJsonDocument(QJsonArray{goal}).toJson());
        backend.loadGoals();
        QTRY_COMPARE(backend.goalHistoryId(), QString("latest"));
        QTRY_VERIFY(firstShown(window, "goalHistoryRow_1"));
        shotWindow(window, "goals-cli-history.png");

        // After the mutation, Goals reads authoritative progress again.
        goal["target"] = 8.0;
        write(dir.filePath("goals.json"), QJsonDocument(QJsonArray{goal}).toJson());
        auto *form = findByName(window, "goalsPageForm");
        QVERIFY(form);
        form->setProperty("metricChoices", QVariantList{QVariantMap{{"id", "focus"}, {"name", "Focus"}}});
        QTRY_VERIFY(firstShown(window, "goalWhat_focus"));
        QVERIFY(QMetaObject::invokeMethod(firstShown(window, "goalWhat_focus"), "clicked"));
        firstShown(window, "goalNewTarget")->setProperty("text", "8");
        QVERIFY(firstShown(window, "goalAddButton")->property("enabled").toBool());
        form->setProperty("metricChoices", QVariantList{});
        QCOMPARE(form->property("metricId").toString(), QString("focus"));
        QVERIFY(!firstShown(window, "goalAddButton")->property("enabled").toBool());
        QVERIFY(hasVisualText(window, "Focus is unavailable. Choose another metric or Minutes."));
        QVERIFY(QMetaObject::invokeMethod(firstShown(window, "goalNewTarget"), "accepted"));
        QVERIFY(offlineRequests(dir).isEmpty());
        shotWindow(window, "goals-unavailable-metric.png");
        QVERIFY(QMetaObject::invokeMethod(firstShown(window, "goalWhat_minutes"), "clicked"));
        QVERIFY(firstShown(window, "goalAddButton")->property("enabled").toBool());
        QVERIFY(QMetaObject::invokeMethod(firstShown(window, "goalAddButton"), "clicked"));
        QTRY_COMPARE(backend.goals().first().toMap().value("target").toDouble(), 8.0);
        QCOMPARE(offlineRequests(dir).size(), 1);
        QVERIFY(!offlineRequests(dir).first().value("params").toObject().contains("metric_id"));

        // Today opens a day row whose effective metrics are available even
        // without the settings catalogue or a current tracking item list.
        window->setProperty("page", 0);
        QVERIFY(QMetaObject::invokeMethod(window, "openEntry", Q_ARG(QVariant, "<id:4>")));
        QTRY_VERIFY(firstShown(window, "goalAddToggle"));
        QVERIFY(QMetaObject::invokeMethod(firstShown(window, "goalAddToggle"), "clicked"));
        QVERIFY(firstShown(window, "goalWhat_<id:1>"));
        shotWindow(window, "task-goal-effective-metrics.png");
    }

    void goalsReloadAtClockBoundary() {
        QTemporaryDir dir;
        FakeDaemon daemon(daemon_path(dir));
        if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        auto state = goalsState();
        daemon.setHandler(trackingHandler(state));
        Backend backend(nullptr, fake(dir), 3000, daemon_path(dir));
        backend.reload();
        waitForBackendIdle(backend);
        QTRY_COMPARE(backend.todayDay(), QString("2031-05-14"));
        Theme theme(dir.path());
        QQmlEngine engine;
        engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl("qrc:/qml/Main.qml"));
        QScopedPointer<QObject> root(component.create());
        QVERIFY2(root, qPrintable(component.errorString()));
        root->setProperty("page", 6);
        QTRY_VERIFY(!backend.goals().isEmpty());
        backend.loadGoalHistory("g1");
        QTRY_COMPARE(backend.goalHistory().size(), 2);
        const int reads = requestCount(daemon, "goals.list");
        const int historyReads = requestCount(daemon, "goals.history");
        state->day["day"] = "2031-05-15";
        daemon.notifyClock("2031-05-15");
        QTRY_COMPARE(backend.todayDay(), QString("2031-05-15"));
        QTRY_COMPARE(requestCount(daemon, "goals.list"), reads + 1);
        QTRY_COMPARE(requestCount(daemon, "goals.history"), historyReads + 1);
        QCOMPARE(lastRequest(daemon, "goals.history").value("params").toObject().value("to").toString(), QString("2031-05-15"));
        daemon.notifyClock("2031-05-15");
        waitForBackendIdle(backend);
        QCOMPARE(requestCount(daemon, "goals.list"), reads + 1);
    }

    void goalsPageAndEditors() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        auto state = goalsState();
        state->backlog = backlogRows();
        daemon.setHandler(trackingHandler(state));
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        Theme theme(dir.path());
        QQmlEngine engine; engine.rootContext()->setContextProperty("backend", &backend);
        engine.rootContext()->setContextProperty("theme", &theme);
        QQmlComponent component(&engine, QUrl::fromLocalFile(QStringLiteral(TEST_SOURCE_DIR "/../src/qml/Main.qml")));
        QScopedPointer<QObject> root(component.create()); QVERIFY2(root, qPrintable(component.errorString()));
        auto *window = qobject_cast<QQuickWindow *>(root.data()); QVERIFY(window);
        window->setWidth(1440); window->setHeight(900); window->show();
        backend.reload();
        waitForBackendIdle(backend);
        QTRY_COMPARE_WITH_TIMEOUT(backend.goals().size(), 6, 3000);
        QTRY_COMPARE_WITH_TIMEOUT(backend.epics().size(), 5, 3000);

        // The sidebar entry opens the Goals page.
        QObject *sidebar = findByName(root.data(), "goalsSidebarItem"); QVERIFY(sidebar);
        auto *sidebarItem = qobject_cast<QQuickItem *>(sidebar); QVERIFY(sidebarItem);
        QTest::mouseClick(window, Qt::LeftButton, {}, sidebarItem->mapToScene(QPointF(sidebarItem->width() / 2, sidebarItem->height() / 2)).toPoint());
        QCOMPARE(root->property("page").toInt(), 6);
        QObject *page = findByName(root.data(), "goalsPage"); QVERIFY(page);
        QTRY_COMPARE_WITH_TIMEOUT(visualNamed(window, "goalRow_g1").size(), 1, 3000);
        QCOMPARE(visualNamed(window, "goalRow_g6").size(), 1);
        // Every row names its scope; the project and epic goals read as such.
        auto titleOf = [&](const QString &id) {
            return findItemByName(visualNamed(window, "goalRow_" + id).value(0), "goalTitle")->property("text").toString();
        };
        auto subtitleOf = [&](const QString &id) {
            return findItemByName(visualNamed(window, "goalRow_" + id).value(0), "goalSubtitle")->property("text").toString();
        };
        QCOMPARE(titleOf("g1"), QString("All work"));
        QCOMPARE(titleOf("g2"), QString("Study"));
        QCOMPARE(titleOf("g3"), QString("Book"));
        QCOMPARE(titleOf("g4"), QString("Read"));
        QCOMPARE(subtitleOf("g1"), QString("Minutes  ·  Day  ·  at least 1h 00m"));
        QCOMPARE(subtitleOf("g6"), QString("Distractions  ·  Week  ·  at most 3 times"));
        auto valueOf = [&](const QString &id) {
            return findItemByName(visualNamed(window, "goalRow_" + id).value(0), "goalValue")->property("text").toString();
        };
        QCOMPARE(valueOf("g1"), QString("1h 02m"));
        QCOMPARE(valueOf("g5"), QString("4.4"));
        QCOMPARE(valueOf("g6"), QString("3 times"));
        QVERIFY(findItemByName(visualNamed(window, "goalRow_g1").value(0), "goalMet")->property("visible").toBool());
        // History for the first goal: one row per period, with the counts.
        QObject *button = findItemByName(visualNamed(window, "goalRow_g1").value(0), "goalHistoryButton"); QVERIFY(button);
        QVERIFY(QMetaObject::invokeMethod(button, "clicked"));
        QTRY_COMPARE_WITH_TIMEOUT(backend.goalHistory().size(), 2, 3000);
        QTRY_VERIFY_WITH_TIMEOUT(!visualNamed(window, "goalHistoryRow_1").isEmpty(), 3000);
        QCOMPARE(visualNamed(window, "goalHistorySummary").value(0)->property("text").toString(), QString("Met in 1 of 2 periods"));
        QCOMPARE(lastRequest(daemon, "goals.history").value("params").toObject().value("id").toString(), QString("g1"));
        shotWindow(window, "goals-page.png");

        // Edit the target inline; the row sends the edit with its version.
        QVERIFY(QMetaObject::invokeMethod(findItemByName(visualNamed(window, "goalRow_g2").value(0), "goalHistoryButton"), "clicked"));
        QVERIFY(QMetaObject::invokeMethod(findItemByName(visualNamed(window, "goalRow_g2").value(0), "goalEditButton"), "clicked"));
        QObject *field = findItemByName(visualNamed(window, "goalRow_g2").value(0), "goalTargetField"); QVERIFY(field);
        QCOMPARE(field->property("text").toString(), QString("50"));
        field->setProperty("text", "55");
        QVERIFY(QMetaObject::invokeMethod(findItemByName(visualNamed(window, "goalRow_g2").value(0), "goalEditSave"), "clicked"));
        QTRY_VERIFY_WITH_TIMEOUT(!lastRequest(daemon, "goal.edit").isEmpty(), 3000);
        QJsonObject params = lastRequest(daemon, "goal.edit").value("params").toObject();
        QCOMPARE(params.value("id").toString(), QString("g2"));
        QCOMPARE(params.value("target").toDouble(), 55.0);
        QTRY_COMPARE(backend.goalHistoryId(), QString("g2-successor"));
        QTRY_COMPARE(lastRequest(daemon, "goals.history").value("params").toObject().value("id").toString(),
                     QString("g2-successor"));
        QTRY_VERIFY(firstShown(window, "goalHistoryRow_1"));
        // Ending asks first.
        QVERIFY(QMetaObject::invokeMethod(findItemByName(visualNamed(window, "goalRow_g3").value(0), "goalEndButton"), "clicked"));
        QCOMPARE(requestCount(daemon, "goal.end"), 0);
        QVERIFY(QMetaObject::invokeMethod(findItemByName(visualNamed(window, "goalRow_g3").value(0), "goalEndConfirm"), "clicked"));
        QTRY_VERIFY_WITH_TIMEOUT(!lastRequest(daemon, "goal.end").isEmpty(), 3000);
        QCOMPARE(lastRequest(daemon, "goal.end").value("params").toObject().value("id").toString(), QString("g3"));
        // The page adds an all-work goal.
        QObject *pageForm = findByName(page, "goalsPageForm"); QVERIFY(pageForm);
        QVERIFY(QMetaObject::invokeMethod(firstShown(window, "goalPeriod_week"), "clicked"));
        QVERIFY(QMetaObject::invokeMethod(firstShown(window, "goalWhat_focus"), "clicked"));
        QObject *add = firstShown(window, "goalAddButton"); QVERIFY(add);
        QVERIFY(!add->property("enabled").toBool());
        QObject *newTarget = firstShown(window, "goalNewTarget"); QVERIFY(newTarget);
        newTarget->setProperty("text", "4");
        QTRY_VERIFY(add->property("enabled").toBool());
        QVERIFY(QMetaObject::invokeMethod(add, "clicked"));
        QTRY_VERIFY_WITH_TIMEOUT(!lastRequest(daemon, "goal.add").isEmpty(), 3000);
        params = lastRequest(daemon, "goal.add").value("params").toObject();
        QCOMPARE(params.value("period").toString(), QString("week"));
        QCOMPARE(params.value("metric_id").toString(), QString("focus"));
        QCOMPARE(params.value("direction").toString(), QString("at_least"));
        QVERIFY(!params.contains("project_id"));

        // The project editor lists its goals and adds one.
        root->setProperty("page", 3);
        QObject *settings = findByName(root.data(), "settingsPage"); QVERIFY(settings);
        settings->setProperty("section", "projects");
        QObject *projectsPane = findByName(root.data(), "projectsSettings"); QVERIFY(projectsPane);
        QVERIFY(QMetaObject::invokeMethod(projectsPane, "startEdit", Q_ARG(QVariant, backend.projects().first())));
        // The Goals page is hidden now; only visible rows count.
        auto shown = [&](const QString &name) {
            int count = 0;
            for (QObject *object : visualNamed(window, name))
                if (qobject_cast<QQuickItem *>(object)->isVisible()) ++count;
            return count;
        };
        QTRY_COMPARE_WITH_TIMEOUT(shown("goalRow_g2-successor"), 1, 3000);
        QCOMPARE(shown("goalRow_g6"), 1);   // Quran's distractions limit
        QCOMPARE(shown("goalRow_g1"), 0);   // all-work goals are not a project's
        QObject *toggle = firstShown(window, "goalAddToggle"); QVERIFY(toggle);
        QVERIFY(QMetaObject::invokeMethod(toggle, "clicked"));
        QVERIFY(QMetaObject::invokeMethod(firstShown(window, "goalWhat_focus"), "clicked"));
        QVERIFY(QMetaObject::invokeMethod(firstShown(window, "goalDirection_at_most"), "clicked"));
        // Only the project's asked metrics are offered (not Distractions here).
        QCOMPARE(shown("goalWhat_pages"), 1);
        QCOMPARE(shown("goalWhat_reps"), 0);
        auto *flick = findByName(projectsPane, "projectEditorFlick");
        if (flick) flick->setProperty("contentY", 400);
        shotWindow(window, "goals-project-editor.png");
        newTarget = firstShown(window, "goalNewTarget"); QVERIFY(newTarget);
        newTarget->setProperty("text", "4,5");
        QVERIFY(QMetaObject::invokeMethod(firstShown(window, "goalAddButton"), "clicked"));
        QTRY_COMPARE_WITH_TIMEOUT(requestCount(daemon, "goal.add"), 2, 3000);
        params = lastRequest(daemon, "goal.add").value("params").toObject();
        QCOMPARE(params.value("project_id").toString(), QString("quran"));
        QCOMPARE(params.value("direction").toString(), QString("at_most"));
        QCOMPARE(params.value("target").toDouble(), 4.5);

        // The epic page and the task editor have the section as well.
        root->setProperty("epicId", "e-juz");
        QVERIFY(QMetaObject::invokeMethod(root.data(), "openEpic", Q_ARG(QVariant, QVariant("e-juz"))));
        QTRY_COMPARE_WITH_TIMEOUT(shown("goalRow_g3"), 1, 3000);
        window->setHeight(1250);  // the whole page, goals included
        shotWindow(window, "goals-epic-page.png");
        window->setHeight(900);
        root->setProperty("page", 0);
        QVERIFY(QMetaObject::invokeMethod(root.data(), "openEntry", Q_ARG(QVariant, QVariant("dhikr"))));
        QTRY_COMPARE_WITH_TIMEOUT(shown("goalRow_g4"), 1, 3000);
        shotWindow(window, "goals-task-editor.png");
        QVERIFY2(backend.error().isEmpty(), qPrintable(backend.error()));
        window->hide();
    }

    void themeFallbackAndAccent() {
        QTemporaryDir dir; Theme fallback(dir.path()); QCOMPARE(fallback.accent(),QString("#FFD60A")); QCOMPARE(fallback.accentForeground(),QString("black"));
        QDir(dir.path()).mkpath("theme"); write(dir.filePath("theme/colors.toml"),"accent = \"#112233\"\n"); Theme themed(dir.path()); QCOMPARE(themed.accent(),QString("#112233")); QCOMPARE(themed.accentForeground(),QString("white"));
    }
    void translatorFallback() {
        QTemporaryDir dir; Translator t(dir.filePath("language"),dir.path()); QCOMPARE(t.language(),QString("en")); QVERIFY(t.translate("Main","Missing").isNull());
        write(dir.filePath("language"),"pt-BR\n"); write(dir.filePath("en.json"),R"({"Save":"Save"})"); write(dir.filePath("pt.json"),R"({"Save":"Guardar"})"); Translator pt(dir.filePath("language"),dir.path()); QCOMPARE(pt.translate("Main","Save"),QString("Guardar"));
    }
};
QTEST_MAIN(SalahAppTests)
#include "salah_app_tests.moc"
