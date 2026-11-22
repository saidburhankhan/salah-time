#include <QDir>
#include <cstring>
#include <QElapsedTimer>
#include <QFileInfo>
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
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
        QVariantMap item;
        for (const QVariant &block : backend.dayPlan().value("blocks").toList())
            for (const QVariant &row : block.toMap().value("items").toList())
                if (row.toMap().value("id").toString() == "<id:1>") item = row.toMap();
        QVERIFY(!item.isEmpty());
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
        auto contains = [&](const QString &id) {
            for (const QVariant &block : backend.dayPlan().value("blocks").toList())
                for (const QVariant &row : block.toMap().value("items").toList())
                    if (row.toMap().value("id").toString() == id) return true;
            return false;
        };
        QTRY_VERIFY_WITH_TIMEOUT(contains("new-1"), 3000);
        for (const QVariant &block : backend.dayPlan().value("blocks").toList())
            for (const QVariant &row : block.toMap().value("items").toList())
                QVERIFY(!row.toMap().value("id").toString().startsWith("pending-"));
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
        auto findItem = [&](const QString &title) {
            for (const QVariant &block : backend.dayPlan().value("blocks").toList())
                for (const QVariant &row : block.toMap().value("items").toList())
                    if (row.toMap().value("title").toString() == title) return row.toMap();
            return QVariantMap{};
        };
        backend.addQuick("Smoke add", "fajr");
        auto canonical = [&](const QString &title) {
            const QVariantMap item = findItem(title);
            return !item.isEmpty() && !item.value("id").toString().startsWith("pending-") && !item.value("pending").toBool();
        };
        QTRY_VERIFY_WITH_TIMEOUT(canonical("Smoke add"), 8000);
        const QVariantMap added = findItem("Smoke add");
        const QString id = added.value("id").toString();
        backend.toggleTimer(id, false);
        QTRY_VERIFY_WITH_TIMEOUT(!findItem("Smoke add").value("timer_started_at").toString().isEmpty(), 8000);
        backend.toggleTimer(id, true);
        QTRY_VERIFY_WITH_TIMEOUT(findItem("Smoke add").value("timer_started_at").toString().isEmpty(), 8000);
        bool marked = false;
        for (const QVariant &row : backend.dayPlan().value("prayers").toList())
            if (row.toMap().value("id").toString() == "fajr") marked = row.toMap().value("marked").toBool();
        backend.togglePrayer("fajr", marked);
        QTRY_VERIFY_WITH_TIMEOUT([&] {
            for (const QVariant &row : backend.dayPlan().value("prayers").toList())
                if (row.toMap().value("id").toString() == "fajr") return row.toMap().value("marked").toBool() != marked;
            return false;
        }(), 8000);
        QProcess external;
        external.start(qEnvironmentVariable("SALAH_BIN", "salah"), {"item", "add", "--json", "--", "External smoke"});
        QVERIFY(external.waitForFinished(8000));
        QCOMPARE(external.exitCode(), 0);
        QTRY_VERIFY_WITH_TIMEOUT(!findItem("External smoke").isEmpty(), 8000);
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
        QVERIFY2(backend.error().isEmpty(),qPrintable(backend.error())); QVERIFY(!backend.stats().isEmpty());
    }
    void asyncQueueAndModels() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir)); QSignalSpy spy(&b,&Backend::requestFinished);
        b.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(),3,3000);
        QCOMPARE(b.todayDay(),QString("2026-11-22")); QCOMPARE(b.dayPlan().value("prayers").toList().size(),5);
        QCOMPARE(b.routinesStats().value("items").toList().first().toMap().value("title").toString(),QString("Read"));
        QCOMPARE(b.config().value("location").toString(),QString("Tashkent"));
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
        b.moveItem("a1","isha",2); b.togglePrayer("fajr",false);
        QTRY_VERIFY_WITH_TIMEOUT(logLines(dir).contains("item move --json a1 --block isha --index 2"),3000);
        QTRY_VERIFY_WITH_TIMEOUT(logLines(dir).contains("prayer mark --json fajr --date 2026-11-22"),3000);
    }
    void rowMoveDestinationAcrossAndEmptyBlocks() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir));
        QVariantList rows{
            QVariantMap{{"kind","header"},{"block","night"}},
            QVariantMap{{"kind","item"},{"block","night"}},
            QVariantMap{{"kind","add"},{"block","night"}},
            QVariantMap{{"kind","header"},{"block","fajr"}},
            QVariantMap{{"kind","item"},{"block","fajr"}},
            QVariantMap{{"kind","item"},{"block","fajr"}},
            QVariantMap{{"kind","add"},{"block","fajr"}},
            QVariantMap{{"kind","header"},{"block","dhuhr"}},
            QVariantMap{{"kind","add"},{"block","dhuhr"}}};
        QCOMPARE(b.moveDestination(rows,6),QVariantMap({{"block","fajr"},{"index",2}}));
        QCOMPARE(b.moveDestination(rows,8),QVariantMap({{"block","dhuhr"},{"index",0}}));
    }
    void unchangedJsonDoesNotResetDayModel() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir)); QSignalSpy spy(&b,&Backend::requestFinished);
        b.reload(); QTRY_COMPARE(spy.size(),3); const int revision=b.dayRevision();
        b.reload(); QTRY_COMPARE(spy.size(),6); QCOMPARE(b.dayRevision(),revision);
    }
    void pendingTickWaitsForCanonicalResponse() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir)); QSignalSpy spy(&b,&Backend::requestFinished);
        b.reload(); QTRY_COMPARE(spy.size(),3); b.toggleDone("a1","done");
        const auto blocks=b.dayPlan().value("blocks").toList();
        const auto row=blocks.at(1).toMap().value("items").toList().first().toMap();
        QCOMPARE(row.value("state").toString(),QString("done"));
        QVERIFY(row.value("pending").toBool());
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
        const auto blocks=b.dayPlan().value("blocks").toList();
        QVERIFY(blocks.at(0).toMap().value("items").toList().first().toMap().value("timer_started_at").toString() == QString("2026-11-22T08:00:00Z"));
        QVERIFY(blocks.at(1).toMap().value("items").toList().first().toMap().value("pending").toBool());
        QVERIFY(blocks.at(1).toMap().value("items").toList().first().toMap().value("timer_started_at").toString().isEmpty());
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
        QCOMPARE(b.stats().value("daily").toList().first().toMap().value("minutes").toInt(),25);
        QCOMPARE(b.stats().value("totals").toMap().value("active_days").toInt(),11);
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
        QVariant tip; QVariantMap day{{"day","2026-11-18"},{"done",3},{"planned",4},{"minutes",95},{"count",30},{"prayers",4}};
        QVERIFY(QMetaObject::invokeMethod(root.data(),"tooltipText",Q_RETURN_ARG(QVariant,tip),Q_ARG(QVariant,day)));
        QCOMPARE(tip.toString(),QString("Wed 18 Nov · 3 / 4 done · 95 min · 30 counts · 4 prayers"));
        QVERIFY(QMetaObject::invokeMethod(root.data(),"tooltipTextAt",Q_RETURN_ARG(QVariant,tip),Q_ARG(QVariant,0)));
        QCOMPARE(tip.toString(),QString("Sun 22 Nov · 1 / 1 done · 25 min · 0 counts · 4 prayers"));
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
