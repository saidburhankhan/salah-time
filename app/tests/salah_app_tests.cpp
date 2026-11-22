#include <QDir>
#include <cstring>
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
    void respondError(QLocalSocket *socket, const QJsonObject &request, int code, const QString &kind, const QString &message) {
        write(socket, QJsonObject{{"jsonrpc", "2.0"}, {"id", request.value("id")},
                                  {"error", QJsonObject{{"code", code}, {"message", message},
                                                        {"data", QJsonObject{{"kind", kind}, {"retryable", false}}}}}});
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
                            {"capabilities", QJsonArray{"timer.v1", "events.v1"}}, {"revision", 100}};
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
)SH") + "fixture='" TEST_SOURCE_DIR "/../../crates/salah-cli/tests/fixtures/contract/'\n" + R"SH(
case "$1 $2" in
  'day --date') cat "${fixture}day.json" ;;
  'stats --days'|'stats --from') cat "${fixture}stats_days.json" ;;
  'config show') cat "${fixture}config_show.json" ;;
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
    static int prayerRow(PrayerModel *model, const QString &id) {
        for (int row = 0; row < model->rowCount(); ++row)
            if (model->data(model->index(row, 0), PrayerModel::PrayerIdRole).toString() == id) return row;
        return -1;
    }
    static QVariantMap itemEditMap(Backend &backend, const QString &id) {
        ItemDraft *draft = backend.itemForId(id);
        QVariantMap item{{"id", draft->id()}, {"title", draft->title()}, {"block", draft->block()},
                         {"repeat", draft->repeat()}, {"measure", draft->measure()},
                         {"target", draft->target()}, {"weekdays", draft->weekdays()}};
        return item;
    }

    static QJsonObject fixture(const QString &name) {
        return readJson(QStringLiteral(TEST_SOURCE_DIR "/../../crates/salah-cli/tests/fixtures/contract/") + name + ".json").object();
    }
    static FakeDaemon::Handler readHandler(QJsonObject day) {
        return [day](const QJsonObject &request, QLocalSocket *socket, FakeDaemon *daemon) {
            const QString method = request.value("method").toString();
            if (method == "day.get") daemon->respond(socket, request, day);
            else if (method == "stats.get") daemon->respond(socket, request, QJsonValue(fixture("stats_days")));
            else if (method == "config.get") daemon->respond(socket, request, QJsonValue(fixture("config_show")));
            else daemon->respond(socket, request, QJsonObject{});
        };
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
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); QVERIFY(daemon.ok());
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
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); QVERIFY(daemon.ok());
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
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); QVERIFY(daemon.ok());
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
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); QVERIFY(daemon.ok());
        QJsonObject day = fixture("day");
        QJsonArray blocks = day.value("blocks").toArray();
        for (int b = 0; b < blocks.size(); ++b) {
            QJsonObject block = blocks[b].toObject();
            QJsonArray items = block.value("items").toArray();
            for (int i = 0; i < items.size(); ++i) {
                QJsonObject item = items[i].toObject();
                if (item.value("id").toString() == "<id:1>") item["version"] = 7;
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
                server->respondError(socket, request, -32004, "conflict", "item \"<id:1>\" is at version 4, not 3");
                return;
            }
            if (request.value("method").toString() == "day.get") ++dayRequests;
            reads(request, socket, server);
        });
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy spy(&backend, &Backend::requestFinished);
        backend.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 3, 3000);
        const int row = timelineItem(backend.dayTimeline(), "<id:1>");
        QVERIFY(row >= 0);
        QVariantMap item = itemEditMap(backend, "<id:1>");
        item["title"] = "Changed";
        backend.saveItem(item);
        QTRY_VERIFY_WITH_TIMEOUT(editRequests == 1, 3000);
        QCOMPARE(editParams.value("expected_version").toInt(), 7);
        QTRY_VERIFY_WITH_TIMEOUT(backend.error().contains("version 4, not 3"), 3000);
        QTRY_VERIFY_WITH_TIMEOUT(dayRequests >= 2, 3000);
    }
    void changedNotificationReloadsOncePerRevision() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); QVERIFY(daemon.ok());
        daemon.setHandler(readHandler(fixture("day")));
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy spy(&backend, &Backend::requestFinished);
        backend.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 3, 3000);
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
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); QVERIFY(daemon.ok());
        auto reads = readHandler(fixture("day"));
        daemon.setHandler([reads](const QJsonObject &request, QLocalSocket *socket, FakeDaemon *server) {
            if (request.value("method").toString() == "item.add") {
                const QJsonObject params = request.value("params").toObject();
                QJsonObject row{{"id", "new-1"}, {"title", params.value("title").toString()}, {"block", "fajr"},
                                {"repeat", "none"}, {"weekdays", QJsonValue::Null}, {"measure", "check"},
                                {"target", QJsonValue::Null}, {"position", 0}, {"state", "open"},
                                {"minutes", 0}, {"count", 0}, {"timer_started_at", QJsonValue::Null}, {"version", 2}};
                QJsonObject changed{{"items", QJsonArray{row}}, {"entries", QJsonArray{}},
                                    {"marks", QJsonArray{}}, {"deleted", QJsonArray{}}};
                server->respond(socket, request, QJsonObject{{"revision", 101}, {"op_id", params.value("op_id")}, {"changed", changed}});
                return;
            }
            reads(request, socket, server);
        });
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy spy(&backend, &Backend::requestFinished);
        backend.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 3, 3000);
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
        const QString fixturePath = QStringLiteral(TEST_SOURCE_DIR "/../../crates/salah-cli/tests/fixtures/contract/config_show.json");
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
        QVERIFY(createStaleSocket(socket));
        QVERIFY(QFileInfo::exists(socket));

        const QString log = dir.filePath("cli-args");
        const QString script = dir.filePath("salah");
        const QString fixturePath = QStringLiteral(TEST_SOURCE_DIR "/../../crates/salah-cli/tests/fixtures/contract/config_show.json");
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
        client.timerStop("id-1", [&](const CoreClient::Result &reply) { result = reply; });
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
        const QString base = QStringLiteral(TEST_SOURCE_DIR "/../../crates/salah-cli/tests/fixtures/contract/");
        auto json = [&](const QString &name) { return readJson(base + name + ".json"); };
        QVERIFY(Dto::decode<Dto::DayPlan>(json("day").object()));
        QVERIFY(Dto::decode<Dto::Today>(json("item_list").object()));
        QVERIFY(Dto::decode<Dto::Stats>(json("stats_range").object()));
        QVERIFY(Dto::decode<Dto::Stats>(json("stats_days").object()));
        QVERIFY(Dto::decode<Dto::ParseDraft>(json("item_parse").object()));
        QVERIFY(Dto::decode<Dto::Config>(json("config_show").object()));
        QVERIFY(Dto::decode<Dto::Item>(json("item_add_plain").object()));
        QVERIFY(Dto::decode<Dto::Entry>(json("item_start").object()));
        QVERIFY(Dto::decode<std::vector<Dto::Entry>>(json("item_stop").array()));
        QVERIFY(Dto::decode<Dto::PrayerMark>(json("prayer_mark").object()));
    }
    void decoderRejectsBadFieldsAndAllowsExtras() {
        const QString base = QStringLiteral(TEST_SOURCE_DIR "/../../crates/salah-cli/tests/fixtures/contract/day.json");
        QFile f(base); QVERIFY(f.open(QIODevice::ReadOnly)); QJsonObject day=QJsonDocument::fromJson(f.readAll()).object();
        day["future_v1_field"] = QJsonObject{{"anything",true}};
        QVERIFY(Dto::decode<Dto::DayPlan>(day));
        QJsonObject missing=day; missing.remove("blocks"); auto a=Dto::decode<Dto::DayPlan>(missing);
        QVERIFY(!a); QCOMPARE(a.error->formatted(),QString("day.blocks: missing required field"));
        QJsonObject mistyped=day; mistyped["open"]="zero"; auto b=Dto::decode<Dto::DayPlan>(mistyped);
        QVERIFY(!b); QCOMPARE(b.error->formatted(),QString("day.open: expected number"));
        QJsonArray blocks=day["blocks"].toArray(); QJsonObject block=blocks[1].toObject(); QJsonArray items=block["items"].toArray();
        QJsonObject item=items[0].toObject(); item["state"]="future"; items[0]=item; block["items"]=items; blocks[1]=block; day["blocks"]=blocks;
        auto c=Dto::decode<Dto::DayPlan>(day); QVERIFY(!c);
        QCOMPARE(c.error->formatted(),QString("day.blocks[1].items[0].state: expected one of open|done|missed"));
    }
    void coreClientDecodesTimerStopArray() {
        QTemporaryDir dir; const QString path=dir.filePath("salah");
        const QString fixture=QStringLiteral(TEST_SOURCE_DIR "/../../crates/salah-cli/tests/fixtures/contract/item_stop.json");
        write(path,"#!/bin/sh\ncat '"+fixture.toUtf8()+"'\n"); QFile::setPermissions(path,QFile::ReadOwner|QFile::WriteOwner|QFile::ExeOwner);
        CoreClient client(nullptr,path,2000,dir.filePath("missing.sock"));
        QSignalSpy spy(&client,&CoreClient::requestFinished); CoreClient::Result result;
        client.timerStop("id-1",[&](const CoreClient::Result&r){result=r;}); QTRY_COMPARE(spy.size(),1);
        QVERIFY2(result.ok, qPrintable(result.error.message)); QVERIFY(std::holds_alternative<std::vector<Dto::Entry>>(result.value));
        QCOMPARE(std::get<std::vector<Dto::Entry>>(result.value).size(),size_t(1));
    }
    void coreClientBuildsEveryCommand() {
        QTemporaryDir dir; CoreClient client(nullptr, fakeCore(dir)); QSignalSpy spy(&client, &CoreClient::requestFinished);
        CoreClient::ItemFields add; add.title = "Title -- literal"; add.block = "asr"; add.repeat = "weekly";
        add.weekdays = {"mon", "wed"}; add.measure = "minutes"; add.target = 30; add.hasTarget = true; add.date = QDate(2026,11,23);
        CoreClient::ItemFields edit = add; edit.title = "New title"; edit.hasTarget = false;
        client.day(QDate(2026,11,22), {}); client.stats(QDate(2026,11,1),QDate(2026,11,30),{});
        client.itemAdd(add); client.itemEdit("id-1",edit); client.itemRemove("id-old"); client.itemMove("id-1","isha",-2);
        client.itemDone("id-1",QDate(2026,11,22)); client.itemUndo("id-1",QDate(2026,11,22));
        client.itemLog("id-1",-15,true,QDate(2026,11,23)); client.itemLog("id-1",-2,false);
        client.timerStart("id-1"); client.timerStop("id-1");
        client.prayerMark("fajr",QDate(2026,11,22)); client.prayerUnmark("fajr",QDate(2026,11,22));
        client.parse("read -- carefully",{}); client.configShow({}); client.configSetSleepTime("22:45");
        QTRY_COMPARE_WITH_TIMEOUT(spy.size(),17,5000);
        const QStringList lines = coreLogLines(dir);
        QCOMPARE(lines.size(), 17);
        QCOMPARE(lines.at(0),"<day><--date><2026-11-22><--json>");
        QCOMPARE(lines.at(1),"<stats><--from><2026-11-01><--to><2026-11-30><--json>");
        QCOMPARE(lines.at(2),"<item><add><--json><--block><asr><--repeat><weekly><--days><mon,wed><--measure><minutes><--target><30><--date><2026-11-23><--><Title -- literal>");
        QCOMPARE(lines.at(3),"<item><edit><--json><id-1><--title=New title><--block><asr><--repeat><weekly><--days><mon,wed><--measure><minutes><--no-target>");
        QCOMPARE(lines.at(4),"<item><remove><--json><id-old>");
        QCOMPARE(lines.at(5),"<item><move><--json><id-1><--block><isha><--index><-2>");
        QCOMPARE(lines.at(6),"<item><done><--json><id-1><--date><2026-11-22>");
        QCOMPARE(lines.at(7),"<item><undo><--json><id-1><--date><2026-11-22>");
        QCOMPARE(lines.at(8),"<item><log><--json><--minutes><id-1><-15><--date><2026-11-23>");
        QCOMPARE(lines.at(9),"<item><log><--json><id-1><-2>");
        QCOMPARE(lines.at(10),"<item><start><--json><id-1>");
        QCOMPARE(lines.at(11),"<item><stop><--json><id-1>");
        QCOMPARE(lines.at(12),"<prayer><mark><--json><fajr><--date><2026-11-22>");
        QCOMPARE(lines.at(13),"<prayer><unmark><--json><fajr><--date><2026-11-22>");
        QCOMPARE(lines.at(14),"<item><parse><--json><--><read -- carefully>");
        QCOMPARE(lines.at(15),"<config><show>");
        QCOMPARE(lines.at(16),"<config><set><--sleep-time><22:45>");
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
        QSignalSpy spy(&backend,&Backend::requestFinished); backend.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(),3,5000);
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
        QSignalSpy spy(&backend,&Backend::requestFinished); backend.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(),3,5000);
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
        b.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(),3,3000);
        QCOMPARE(b.todayDay(),QString("2026-11-22")); QCOMPARE(b.prayers()->rowCount(),5);
        QCOMPARE(b.routines()->data(b.routines()->index(0, 0), StatsItemsModel::TitleRole).toString(),QString("Read"));
        QCOMPARE(b.config()->location(),QString("Tashkent"));
        QCOMPARE(logLines(dir).mid(0,3),QStringList({"day --json","stats --days 14 --json","config show"}));
    }
    void timeoutReportsAndQueueContinues() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir),100); QSignalSpy spy(&b,&Backend::requestFinished);
        b.parseQuick("slow"); b.reload(); QTRY_VERIFY_WITH_TIMEOUT(spy.size()>=4,3000); QVERIFY(b.error().contains("timed out"));
    }
    void exactWriteArguments() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir)); QSignalSpy spy(&b,&Backend::requestFinished);
        b.reload(); QTRY_COMPARE(spy.size(),3); b.addQuick("Read Quran after Isha","asr");
        QTRY_VERIFY_WITH_TIMEOUT(logLines(dir).contains("item add --json --quick --block asr -- Read Quran after Isha"),2000);
        b.moveItem("a1","isha"); b.togglePrayer("fajr",false);
        QTRY_VERIFY_WITH_TIMEOUT(logLines(dir).contains("item move --json a1 --block isha --index 0"),3000);
        QTRY_VERIFY_WITH_TIMEOUT(logLines(dir).contains("prayer mark --json fajr --date 2026-11-22"),3000);
    }
    void socketMoveSendsNeighbourAnchors() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); QVERIFY(daemon.ok());
        auto reads = readHandler(fixture("day"));
        daemon.setHandler([reads](const QJsonObject &request, QLocalSocket *socket, FakeDaemon *server) {
            if (request.value("method").toString() == "item.move") {
                server->respond(socket, request, QJsonObject{{"revision", 101},
                    {"op_id", request.value("params").toObject().value("op_id")},
                    {"changed", QJsonObject{{"items", QJsonArray{}}, {"entries", QJsonArray{}},
                                            {"marks", QJsonArray{}}, {"deleted", QJsonArray{}}}}});
                return;
            }
            reads(request, socket, server);
        });
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy spy(&backend, &Backend::requestFinished);
        backend.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 3, 3000);
        backend.moveItem("<id:1>", "asr", "<id:2>", {});
        auto moveSent = [&] {
            for (const QJsonObject &request : std::as_const(daemon.requests))
                if (request.value("method").toString() == "item.move") return true;
            return false;
        };
        QTRY_VERIFY_WITH_TIMEOUT(moveSent(), 3000);
        QJsonObject moveParams;
        for (const QJsonObject &request : std::as_const(daemon.requests))
            if (request.value("method").toString() == "item.move") moveParams = request.value("params").toObject();
        QCOMPARE(moveParams.value("block").toString(), QString("asr"));
        QCOMPARE(moveParams.value("after_id").toString(), QString("<id:2>"));
        QVERIFY(!moveParams.contains("before_id"));
        QVERIFY(!moveParams.contains("index"));
    }
    void unchangedJsonDoesNotResetDayModel() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir)); QSignalSpy spy(&b,&Backend::requestFinished);
        b.reload(); QTRY_COMPARE(spy.size(),3);
        auto *model = b.dayTimeline();
        QSignalSpy changed(model, &QAbstractItemModel::dataChanged);
        QSignalSpy inserted(model, &QAbstractItemModel::rowsInserted);
        QSignalSpy removed(model, &QAbstractItemModel::rowsRemoved);
        QSignalSpy moved(model, &QAbstractItemModel::rowsMoved);
        QSignalSpy reset(model, &QAbstractItemModel::modelReset);
        b.reload(); QTRY_COMPARE(spy.size(),6); QTest::qWait(50);
        QCOMPARE(changed.size(), 0); QCOMPARE(inserted.size(), 0);
        QCOMPARE(removed.size(), 0); QCOMPARE(moved.size(), 0); QCOMPARE(reset.size(), 0);
    }
    void pendingTickWaitsForCanonicalResponse() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir)); QSignalSpy spy(&b,&Backend::requestFinished);
        b.reload(); QTRY_COMPARE(spy.size(),3); b.toggleDone("a1","done");
        const int row = timelineItem(b.dayTimeline(), "a1");
        QVERIFY(row >= 0);
        QCOMPARE(b.dayTimeline()->data(b.dayTimeline()->index(row, 0), DayTimelineModel::StateRole).toString(),QString("done"));
        QVERIFY(b.dayTimeline()->data(b.dayTimeline()->index(row, 0), DayTimelineModel::PendingRole).toBool());
        QVERIFY(b.busy());
    }
    void adjustmentArgumentsAndClamp() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir)); QSignalSpy spy(&b,&Backend::requestFinished);
        b.reload(); QTRY_COMPARE(spy.size(),3);
        b.adjustMinutes("a1",-10); b.adjustMinutes("a1",5); b.adjustCount("n1",1); b.adjustCount("n1",-1);
        QTRY_VERIFY_WITH_TIMEOUT(logLines(dir).contains("item log --json --minutes a1 -10"),3000);
        QTRY_VERIFY_WITH_TIMEOUT(logLines(dir).contains("item log --json --minutes a1 5"),3000);
        QTRY_VERIFY_WITH_TIMEOUT(logLines(dir).contains("item log --json n1 1"),3000);
        QTRY_VERIFY_WITH_TIMEOUT(logLines(dir).contains("item log --json n1 -1"),3000);
        b.selectDay("2026-11-23");
        b.adjustMinutes("a1",5); b.adjustMinutes("a1",-5);
        QTRY_VERIFY_WITH_TIMEOUT(logLines(dir).contains("item log --json --minutes a1 5 --date 2026-11-23"),3000);
        QTRY_VERIFY_WITH_TIMEOUT(logLines(dir).contains("item log --json --minutes a1 -5 --date 2026-11-23"),3000);
    }
    void timerStartMarksOnlyTheTouchedRowPending() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir)); QSignalSpy spy(&b,&Backend::requestFinished);
        b.reload(); QTRY_COMPARE(spy.size(),3); b.toggleTimer("a1",false);
        const int n1 = timelineItem(b.dayTimeline(), "n1");
        const int a1 = timelineItem(b.dayTimeline(), "a1");
        QVERIFY(n1 >= 0); QVERIFY(a1 >= 0);
        QCOMPARE(b.dayTimeline()->data(b.dayTimeline()->index(n1, 0), DayTimelineModel::TimerStartedAtRole).toString(),
                 QString("2026-11-22T08:00:00Z"));
        QVERIFY(b.dayTimeline()->data(b.dayTimeline()->index(a1, 0), DayTimelineModel::PendingRole).toBool());
        QVERIFY(b.dayTimeline()->data(b.dayTimeline()->index(a1, 0), DayTimelineModel::TimerStartedAtRole).toString().isEmpty());
        b.toggleTimer("a1",true);
        QTRY_VERIFY_WITH_TIMEOUT(logLines(dir).contains("item stop --json a1"),3000);
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
        QCOMPARE(entry->property("days").toList(),QVariantList({"sun"})); QVERIFY(entry->property("weeklyValid").toBool());
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
        QCOMPARE(b.statsDaily()->data(b.statsDaily()->index(0, 0), StatsDailyModel::MinutesRole).toInt(),25);
        QCOMPARE(b.statsTotals()->activeDays(),11);
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
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); QVERIFY(daemon.ok());
        QJsonObject day = fixture("day");
        QJsonObject now = day.value("now").toObject();
        QJsonObject next = now.value("next").toObject();
        next["at"] = "2031-05-14T07:19:00Z";
        now["next"] = next; day["now"] = now;
        QJsonArray blocks = day.value("blocks").toArray();
        for (int b = 0; b < blocks.size(); ++b) {
            QJsonObject block = blocks[b].toObject();
            QJsonArray items = block.value("items").toArray();
            for (int i = 0; i < items.size(); ++i) {
                QJsonObject row = items[i].toObject();
                if (row.value("id").toString() == "<id:1>") {
                    row["schedule_text"] = "CORE Daily";
                    row["detail_text"] = "CORE 0 / 30 min · CORE Daily";
                    row["next_occurrence"] = "2031-05-14";
                    row["carried"] = true;
                    row["timer"] = QJsonObject{{"running", false}, {"started_at", QJsonValue::Null},
                                               {"minutes_including_running", 0}};
                } else {
                    row["schedule_text"] = "CORE Mon, Wed";
                    row["detail_text"] = "CORE Mon, Wed";
                    row["next_occurrence"] = "2031-05-19";
                    row["carried"] = false;
                    row["timer"] = QJsonObject{{"running", false}, {"started_at", QJsonValue::Null},
                                               {"minutes_including_running", 0}};
                }
                items[i] = row;
            }
            block["items"] = items; blocks[b] = block;
        }
        day["blocks"] = blocks;
        QJsonObject stats = fixture("stats_days");
        QJsonArray daily = stats.value("daily").toArray();
        QJsonObject dailyRow = daily.last().toObject();
        dailyRow["tooltip"] = "CORE TOOLTIP";
        daily[daily.size() - 1] = dailyRow; stats["daily"] = daily;
        stats["time_ticks"] = QJsonArray{
            QJsonObject{{"minutes", 0}, {"label", "0"}},
            QJsonObject{{"minutes", 30}, {"label", "30m"}},
            QJsonObject{{"minutes", 60}, {"label", "1h"}},
            QJsonObject{{"minutes", 90}, {"label", "1h30"}}};
        QJsonArray items = stats.value("items").toArray();
        QJsonObject statsItem = items.first().toObject();
        statsItem["schedule_text"] = "CORE ROUTINE";
        statsItem["next_occurrence"] = "2031-05-19";
        items[0] = statsItem; stats["items"] = items;
        daemon.setHandler([day, stats](const QJsonObject &request, QLocalSocket *socket, FakeDaemon *server) {
            const QString method = request.value("method").toString();
            if (method == "day.get") server->respond(socket, request, day);
            else if (method == "stats.get") server->respond(socket, request, stats);
            else if (method == "config.get") server->respond(socket, request, QJsonValue(fixture("config_show")));
            else if (method == "item.preview") server->respond(socket, request, QJsonObject{
                {"next_occurrence", "2031-05-19"}, {"occurrences_this_month", 7},
                {"schedule_text", "CORE PREVIEW"}});
            else server->respond(socket, request, QJsonObject{});
        });
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy spy(&backend, &Backend::requestFinished);
        backend.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 3, 3000);
        const int row = timelineItem(backend.dayTimeline(), "<id:1>");
        QVERIFY(row >= 0);
        auto role = [&](int r) { return backend.dayTimeline()->data(backend.dayTimeline()->index(row, 0), r); };
        QCOMPARE(role(DayTimelineModel::DetailTextRole).toString(), QString("CORE 0 / 30 min · CORE Daily"));
        QCOMPARE(role(DayTimelineModel::NextOccurrenceRole).toString(), QString("2031-05-14"));
        QCOMPARE(role(DayTimelineModel::TimerMinutesRole).toInt(), 0);
        backend.loadStatsRange("2031-05-08", "2031-05-14");
        QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 4, 3000);
        QCOMPARE(backend.statsTicks()->rowCount(), 4);

        auto decodedDay = Dto::decode<Dto::DayPlan>(day);
        QVERIFY(decodedDay);
        QCOMPARE(decodedDay.value->blocks[1].items[0].schedule_text, QString("CORE Daily"));
        QCOMPARE(decodedDay.value->blocks[1].items[0].timer_minutes_including_running, qint64(0));
        QCOMPARE(decodedDay.value->now->next_at, QString("2031-05-14T07:19:00Z"));
        auto decodedStats = Dto::decode<Dto::Stats>(stats);
        QVERIFY(decodedStats);
        QCOMPARE(decodedStats.value->time_ticks.size(), size_t(4));
        QCOMPARE(decodedStats.value->daily.back().tooltip, QString("CORE TOOLTIP"));
        auto decodedPreview = Dto::decode<Dto::Preview>(QJsonObject{
            {"next_occurrence", "2031-05-19"}, {"occurrences_this_month", 7},
            {"schedule_text", "CORE PREVIEW"}});
        QVERIFY(decodedPreview);
        QCOMPARE(decodedPreview.value->occurrences_this_month, 7);

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
        backend.reload();
        QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 7, 3000);
        QTest::qWait(200);

        QQmlComponent cardComponent(&engine, QUrl::fromLocalFile(QStringLiteral(TEST_SOURCE_DIR "/../src/qml/today/ItemCard.qml")));
        QScopedPointer<QObject> card(cardComponent.createWithInitialProperties(
            {{"win", QVariant::fromValue(root.data())},
             {"itemId", role(DayTimelineModel::ItemIdRole)},
             {"title", role(DayTimelineModel::TitleRole)},
             {"state", role(DayTimelineModel::StateRole)},
             {"measure", role(DayTimelineModel::MeasureRole)},
             {"detailText", role(DayTimelineModel::DetailTextRole)},
             {"pending", role(DayTimelineModel::PendingRole)},
             {"timerRunning", role(DayTimelineModel::TimerRunningRole)},
             {"timerStartedAt", role(DayTimelineModel::TimerStartedAtRole)},
             {"timerMinutes", role(DayTimelineModel::TimerMinutesRole)},
             {"timerProjected", role(DayTimelineModel::TimerProjectedRole)},
             {"count", role(DayTimelineModel::CountRole)},
             {"target", role(DayTimelineModel::TargetRole)}}));
        QVERIFY2(card, qPrintable(cardComponent.errorString()));
        QTest::qWait(50);
        QVERIFY(hasLabelText(card.data(), "CORE 0 / 30 min · CORE Daily"));

        backend.previewItem(QVariantMap{{"block", "asr"}, {"repeat", "weekly"},
            {"weekdays", QVariantList{"mon", "wed", "fri"}}, {"measure", "check"}, {"target", 0}});
        QTRY_COMPARE_WITH_TIMEOUT(backend.preview()->scheduleText(), QString("CORE PREVIEW"), 3000);
        QObject *entry = root->findChild<QObject *>("entryPopup"); QVERIFY(entry);
        root->setProperty("entryVisible", true); entry->setProperty("visible", true);
        QTRY_VERIFY_WITH_TIMEOUT(hasLabelText(root.data(), "CORE PREVIEW"), 3000);
        QVERIFY(hasLabelText(root.data(), "May 19, 2031"));
        root->setProperty("visible", false);
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
        auto *child = qobject_cast<StatsSeriesModel *>(
            items.data(items.index(0, 0), StatsItemsModel::SeriesRole).value<QObject *>());
        QVERIFY(child);
        QAbstractItemModelTester childTester(child, QAbstractItemModelTester::FailureReportingMode::QtTest);
        QVERIFY(child->rowCount() > 0);

        RoutinesModel routines;
        QAbstractItemModelTester routinesTester(&routines, QAbstractItemModelTester::FailureReportingMode::QtTest);
        routines.setStats(*stats.value);
        for (int row = 0; row < routines.rowCount(); ++row)
            QVERIFY(routines.data(routines.index(row, 0), StatsItemsModel::RepeatRole).toString() != "none");

        StatsTicksModel ticks;
        QAbstractItemModelTester ticksTester(&ticks, QAbstractItemModelTester::FailureReportingMode::QtTest);
        ticks.setTicks(stats.value->time_ticks);
        QCOMPARE(ticks.rowCount(), int(stats.value->time_ticks.size()));
        if (!stats.value->time_ticks.empty())
            QCOMPARE(ticks.lastMinutes(), int(stats.value->time_ticks.back().minutes));
    }

    void timelineOrdersItemsByBlockAndPosition() {
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); QVERIFY(daemon.ok());
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
        backend.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 3, 3000);
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
        QTemporaryDir dir; FakeDaemon daemon(dir.filePath("salah.sock")); QVERIFY(daemon.ok());
        QJsonObject day = fixture("day");
        QJsonObject edited;
        for (const QJsonValue &blockValue : day.value("blocks").toArray()) {
            for (const QJsonValue &itemValue : blockValue.toObject().value("items").toArray()) {
                if (itemValue.toObject().value("id").toString() == "<id:1>") edited = itemValue.toObject();
            }
        }
        QVERIFY(!edited.isEmpty());
        auto reads = readHandler(day);
        daemon.setHandler([reads, edited](const QJsonObject &request, QLocalSocket *socket, FakeDaemon *server) mutable {
            if (request.value("method").toString() == "item.edit") {
                QJsonObject row = edited;
                row["title"] = "Mutated";
                row["version"] = 8;
                const QJsonObject changed{{"items", QJsonArray{row}}, {"entries", QJsonArray{}},
                                          {"marks", QJsonArray{}}, {"deleted", QJsonArray{}}};
                server->respond(socket, request, QJsonObject{
                    {"revision", 101}, {"op_id", request.value("params").toObject().value("op_id")},
                    {"changed", changed}});
                return;
            }
            reads(request, socket, server);
        });
        Backend backend(nullptr, QString(), 3000, daemon_path(dir));
        QSignalSpy spy(&backend, &Backend::requestFinished);
        backend.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(), 3, 3000);
        QSignalSpy changed(backend.dayTimeline(), &QAbstractItemModel::dataChanged);
        QSignalSpy reset(backend.dayTimeline(), &QAbstractItemModel::modelReset);
        QVariantMap item = itemEditMap(backend, "<id:1>");
        item["title"] = "Mutated";
        backend.saveItem(item);
        QTRY_VERIFY_WITH_TIMEOUT(timelineItem(backend.dayTimeline(), "<id:1>") >= 0, 3000);
        QTRY_VERIFY_WITH_TIMEOUT(backend.dayTimeline()->data(
            backend.dayTimeline()->index(timelineItem(backend.dayTimeline(), "<id:1>"), 0),
            DayTimelineModel::TitleRole).toString() == "Mutated", 3000);
        QVERIFY(changed.size() >= 1);
        for (const QList<QVariant> &args : changed) {
            QCOMPARE(args.at(0).toModelIndex().row(), args.at(1).toModelIndex().row());
        }
        QCOMPARE(reset.size(), 0);
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
