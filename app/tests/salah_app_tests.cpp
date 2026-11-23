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
#include <QtTest>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>
#include "backend.h"
#include "client/coreclient.h"
#include "client/dto.h"
#include "models/statsseriesmodel.h"
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
}

class SalahAppTests : public QObject {
    Q_OBJECT
    static void write(const QString &path, const QByteArray &data) { QFile f(path); QVERIFY(f.open(QIODevice::WriteOnly)); QCOMPARE(f.write(data), data.size()); }
    static QJsonDocument readJson(const QString &path) { QFile f(path); if (!f.open(QIODevice::ReadOnly)) qFatal("fixture not readable"); return QJsonDocument::fromJson(f.readAll()); }
    static QString fake(QTemporaryDir &dir) {
        const QString path=dir.filePath("salah"), log=dir.filePath("args");
        QByteArray script = "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '"+log.toUtf8()+"'\n";
        script += "rpc='" TEST_SOURCE_DIR "/../../crates/salah-cli/tests/fixtures/rpc/'\n"
                  "if [ \"$1\" = day ]; then jq -c .result \"${rpc}day_tracking.json\"; exit 0; fi\n"
                  "if [ \"$1\" = stats ]; then jq -c .result \"${rpc}stats.json\"; exit 0; fi\n";
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
        qint64 revision = 100;
    };
    static QJsonObject emptyChanged() {
        return QJsonObject{{"items", QJsonArray{}}, {"sessions", QJsonArray{}}, {"projects", QJsonArray{}},
                           {"tags", QJsonArray{}}, {"metrics", QJsonArray{}}, {"marks", QJsonArray{}},
                           {"deleted", QJsonArray{}}};
    }
    static FakeDaemon::Handler trackingHandler(std::shared_ptr<TrackingState> state) {
        return [state](const QJsonObject &request, QLocalSocket *socket, FakeDaemon *server) {
            const QString method = request.value("method").toString();
            const QJsonObject params = request.value("params").toObject();
            if (method == "day.get") server->respond(socket, request, state->day);
            else if (method == "status.get") server->respond(socket, request, state->status);
            else if (method == "stats.get") {
                const int days = params.value("days").toInt(28);
                server->respond(socket, request, TrackingFixture::stats(params.value("tag_id").toString(),
                                                                        params.value("project_id").toString(),
                                                                        days > 0 ? days : 28));
            }
            else if (method == "config.get") server->respond(socket, request, QJsonObject{{"sleep_time", "23:00"}, {"location", "Tashkent"}, {"latitude", 41.2}, {"longitude", 69.2}, {"timezone_offset", 5}, {"method", "uzbekistan"}, {"method_name", "Muslim Board of Uzbekistan"}, {"asr", "hanafi"}, {"auto_location", true}, {"last_location_provider", QJsonValue::Null}, {"last_location_sync", QJsonValue::Null}});
            else if (method == "projects.list") server->respond(socket, request, TrackingFixture::projects());
            else if (method == "tags.list") server->respond(socket, request, TrackingFixture::tags());
            else if (method == "metrics.list") server->respond(socket, request, TrackingFixture::metrics());
            else if (method == "item.preview") server->respond(socket, request, fixture("item_preview"));
            else if (params.contains("op_id")) {
                QJsonObject changed = emptyChanged();
                if (method == "project.add" || method == "project.edit")
                    changed["projects"] = QJsonArray{QJsonObject{{"id", params.value("id").toString("new-project")},
                                                                 {"name", params.value("name")}}};
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
    bool m_integrationEnvironment = false;

private slots:
    void initTestCase() {
        // The app runs with the Material style (main.cpp); render the same.
        QQuickStyle::setStyle("Material");
        // The opt-in integration tests below intentionally use the caller's
        // desktop environment; everything else runs in the sandbox set up in
        // init().
        m_integrationEnvironment = !qEnvironmentVariableIsEmpty("SALAH_PROTOCOL_SMOKE") ||
                                   !qEnvironmentVariableIsEmpty("SALAH_APP_SMOKE") ||
                                   !qEnvironmentVariableIsEmpty("A9_SCREENSHOT_DIR");
    }
    void init() {
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
        QCOMPARE(daemon.requests.at(1).value("params").toObject().value("topics").toArray().first().toString(), QString("changes"));
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
        write(path,"#!/bin/sh\njq -c .result '"+fixture.toUtf8()+"'\n"); QFile::setPermissions(path,QFile::ReadOwner|QFile::WriteOwner|QFile::ExeOwner);
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
        client.sessionLog("id-1", "project-1", 15, {}, {}, QDate(2026,11,23));
        client.sessionLog("id-1", "project-1");
        client.sessionStart("id-1", "project-1"); client.sessionStop("session-1");
        client.prayerMark("fajr",QDate(2026,11,22)); client.prayerUnmark("fajr",QDate(2026,11,22));
        client.parse("read -- carefully",{}); client.configShow({}); client.configSetSleepTime("22:45");
        client.sessionStop("session-2", QJsonObject{{"focus", 4}}, "Finished chapter", {}, true, 32);
        QTRY_COMPARE_WITH_TIMEOUT(spy.size(),16,5000);
        const QStringList lines = coreLogLines(dir);
        QCOMPARE(lines.size(), 16);
        QCOMPARE(lines.at(0),"<day><--date><2026-11-22><--json>");
        QCOMPARE(lines.at(1),"<stats><--from><2026-11-01><--to><2026-11-30><--json>");
        QCOMPARE(lines.at(2),"<item><add><--json><--block><asr><--repeat><weekly><--days><mon,wed><--project><project-1><--planned-minutes><30><--date><2026-11-23><--><Title -- literal>");
        QCOMPARE(lines.at(3),"<item><edit><--json><id-1><--title=New title><--block><asr><--repeat><weekly><--days><mon,wed><--project><project-1><--no-planned-minutes>");
        QCOMPARE(lines.at(4),"<item><remove><--json><id-old>");
        QCOMPARE(lines.at(5),"<item><move><--json><id-1><--block><isha><--index><-2>");
        QCOMPARE(lines.at(6),"<session><log><--json>");
        QCOMPARE(lines.at(7),"<session><log><--json>");
        QCOMPARE(lines.at(8),"<session><start><--json>");
        QCOMPARE(lines.at(9),"<session><stop><--json><--id><session-1>");
        QCOMPARE(lines.at(10),"<prayer><mark><--json><fajr><--date><2026-11-22>");
        QCOMPARE(lines.at(11),"<prayer><unmark><--json><fajr><--date><2026-11-22>");
        QCOMPARE(lines.at(12),"<item><parse><--json><--><read -- carefully>");
        QCOMPARE(lines.at(13),"<config><show>");
        QCOMPARE(lines.at(14),"<config><set><--sleep-time><22:45>");
        QCOMPARE(lines.at(15), "<session><stop><--json><--id><session-2><--value><focus=4>"
                               "<--note=Finished chapter><--done><--minutes><32>");
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
        QCOMPARE(logLines(dir).mid(0,3),QStringList({"day --json","stats --days 14 --json","config show"}));
    }
    void timeoutReportsAndQueueContinues() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir),100); QSignalSpy spy(&b,&Backend::requestFinished);
        b.parseQuick("slow"); b.reload(); QTRY_VERIFY_WITH_TIMEOUT(spy.size()>=4,3000); QVERIFY(b.error().contains("timed out"));
    }
    void exactWriteArguments() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir)); QSignalSpy spy(&b,&Backend::requestFinished);
        b.reload(); QTRY_COMPARE(spy.size(),4); b.addQuick("Read Quran after Isha","asr");
        QTRY_VERIFY_WITH_TIMEOUT(logLines(dir).contains("item add --json --quick --block asr -- Read Quran after Isha"),2000);
        b.moveItem("a1","isha"); b.togglePrayer("fajr",false);
        QTRY_VERIFY_WITH_TIMEOUT(logLines(dir).contains("item move --json a1 --block isha --index 0"),3000);
        QTRY_VERIFY_WITH_TIMEOUT(logLines(dir).contains("prayer mark --json fajr --date 2031-05-14"),3000);
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
        QTRY_VERIFY_WITH_TIMEOUT(logLines(dir).contains("session start --json"),3000);
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
        root->setProperty("statsMode","month"); root->setProperty("statsAnchor",QDateTime(QDate(2026,1,31),QTime()));
        QVERIFY(QMetaObject::invokeMethod(root.data(),"stepStats",Q_ARG(QVariant,1))); QCOMPARE(root->property("statsAnchor").toDateTime().date(),QDate(2026,2,1));
        root->setProperty("statsMode","week"); root->setProperty("statsAnchor",QDateTime(QDate(2026,11,22),QTime()));
        QVERIFY(QMetaObject::invokeMethod(root.data(),"stepStats",Q_ARG(QVariant,-1))); QCOMPARE(root->property("statsAnchor").toDateTime().date(),QDate(2026,11,15));
        root->setProperty("statsMode","custom"); root->setProperty("customFrom",QDateTime(QDate(2026,11,29),QTime())); root->setProperty("customTo",QDateTime(QDate(2026,12,2),QTime()));
        QVERIFY(QMetaObject::invokeMethod(root.data(),"stepStats",Q_ARG(QVariant,1))); QCOMPARE(root->property("customFrom").toDateTime().date(),QDate(2026,12,3)); QCOMPARE(root->property("customTo").toDateTime().date(),QDate(2026,12,6));
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
        routines.setStats(*stats.value);
        for (int row = 0; row < routines.rowCount(); ++row)
            QVERIFY(routines.data(routines.index(row, 0), StatsItemsModel::RepeatRole).toString() != "none");

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
        QTRY_COMPARE_WITH_TIMEOUT(settings.size(), 3, 3000);
        QCOMPARE(backend.projects().size(), 4);
        QCOMPARE(backend.tags().size(), 3);
        QCOMPARE(backend.metrics().size(), 6);
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

        // Ticking an item whose timer runs pauses it first (no values).
        backend.tickItem("deep", QVariantMap{{"focus", 4}});
        QTRY_COMPARE_WITH_TIMEOUT(requestCount(daemon, "session.log"), 2, 3000);
        int stopAt = -1, logAt = -1;
        for (int i = 0; i < daemon.requests.size(); ++i) {
            const QString method = daemon.requests.at(i).value("method").toString();
            if (method == "session.stop") stopAt = i;
            if (method == "session.log") logAt = i;
        }
        QVERIFY(stopAt >= 0 && stopAt < logAt);
        const QJsonObject stop = daemon.requests.at(stopAt).value("params").toObject();
        QCOMPARE(stop.value("id").toString(), QString("s-run"));
        QVERIFY(!stop.contains("values"));

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
    void trackingUiScreens() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); if (!daemon.ok()) QSKIP("AF_UNIX unavailable in sandbox");
        daemon.setHandler(trackingHandler(trackingState()));
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
        settings->setProperty("section", "tags");
        shotWindow(window, "settings-tags.png");
        settings->setProperty("section", "metrics");
        QObject *metricsPane = findByName(root.data(), "metricsSettings"); QVERIFY(metricsPane);
        QVERIFY(QMetaObject::invokeMethod(metricsPane, "startEdit", Q_ARG(QVariant, backend.metrics().at(2))));
        shotWindow(window, "settings-metrics.png");

        root->setProperty("page", 0);
        QVERIFY(QMetaObject::invokeMethod(root.data(), "openEntry", Q_ARG(QVariant, QVariant("deep"))));
        shotWindow(window, "item-editor.png");
        root->setProperty("entryVisible", false);

        // Today with the session lists of a timed, a ticked and a running item open.
        QObject *timeline = nullptr;
        for (QObject *object : root->findChildren<QObject *>())
            if (object->property("expanded").isValid() && object->metaObject()->indexOfMethod("toggleExpanded(QVariant)") >= 0) timeline = object;
        QVERIFY(timeline);
        for (const char *id : {"quran", "walk", "deep"})
            QVERIFY(QMetaObject::invokeMethod(timeline, "toggleExpanded", Q_ARG(QVariant, QVariant(id))));
        shotWindow(window, "today-sessions.png");
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
        QObject *pause = findByName(root.data(), "runningStopButton"); QVERIFY(pause);
        QVERIFY(QMetaObject::invokeMethod(pause, "clicked"));
        QTRY_VERIFY_WITH_TIMEOUT(prompt->property("visible").toBool(), 2000);
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

        QVERIFY(showPrompt(QVariantMap{
            {"mode", "log"}, {"itemId", "walk"}, {"title", "Morning walk"}}));
        QTRY_VERIFY_WITH_TIMEOUT(prompt->property("visible").toBool(), 2000);
        // No project: the built-in Focus and Quality.
        QCOMPARE(prompt->property("metrics").toList().size(), 2);
        QCOMPARE(prompt->property("metrics").toList().first().toMap().value("metric_id").toString(), QString("focus"));
        root->setProperty("sessionPromptVisible", false);
        // Statistics: all, one project, 90 days, scrolled to the breakdowns.
        root->setProperty("page", 2);
        QVERIFY(QMetaObject::invokeMethod(root.data(), "loadStatistics"));
        waitForBackendIdle(backend);
        shotWindow(window, "stats-all.png");
        QObject *stats = findByName(root.data(), "statsScroll"); QVERIFY(stats);
        QObject *flick = stats->property("contentItem").value<QObject *>(); QVERIFY(flick);
        flick->setProperty("contentY", 520);
        shotWindow(window, "stats-all-bottom.png");
        flick->setProperty("contentY", 0);
        QVERIFY(QMetaObject::invokeMethod(stats, "setFilter", Q_ARG(QVariant, QVariant("project")), Q_ARG(QVariant, QVariant("quran"))));
        waitForBackendIdle(backend);
        QCOMPARE(lastRequest(daemon, "stats.get").value("params").toObject().value("project_id").toString(), QString("quran"));
        shotWindow(window, "stats-project.png");
        QVERIFY(QMetaObject::invokeMethod(stats, "setFilter", Q_ARG(QVariant, QVariant("")), Q_ARG(QVariant, QVariant(""))));
        QVERIFY(QMetaObject::invokeMethod(stats, "setRange", Q_ARG(QVariant, QVariant(90))));
        waitForBackendIdle(backend);
        QCOMPARE(lastRequest(daemon, "stats.get").value("params").toObject().value("days").toInt(), 90);
        shotWindow(window, "stats-90.png");

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
