#pragma once

#include <QDate>
#include <QJsonObject>
#include <QObject>
#include <QProcess>
#include <QQueue>
#include <QTimer>
#include <functional>
#include "dto.h"
#include "sockettransport.h"

// The app's core client. It speaks protocol v1 JSON-RPC over the daemon
// socket when reachable. Offline writes use the same frames over stdio RPC.
class CoreClient : public QObject {
    Q_OBJECT
public:
    enum class ErrorKind { Process, Exit, Decode, Timeout, Conflict, Rpc };
    struct Error {
        ErrorKind kind;
        QString message;
        // `error.data` of a JSON-RPC error, e.g. {"kind":"validation","missing":[...]}.
        QJsonObject data = {};
    };
    struct Result {
        Dto::Value value;
        Error error{ErrorKind::Process, {}};
        bool ok = false;
        QString opId;
    };
    struct ItemFields {
        QString title;
        QString block = "fajr";
        QString repeat = "none";
        QStringList weekdays;
        QString projectId;
        // Undefined leaves the epic alone, null clears it, a string sets it.
        QJsonValue epicId = QJsonValue::Undefined;
        // Undefined preserves the current selection, null follows the project.
        QJsonValue metricIds = QJsonValue::Undefined;
        int plannedMinutes = 0;
        bool hasPlannedMinutes = false;
        QDate date;
        bool quick = false;
        bool backlog = false;
        // Optional caller-supplied idempotency key (used by the UI to map a
        // pending create to its canonical row). Empty means generate one.
        QString opId;
    };
    using Callback = std::function<void(const Result &)>;

    explicit CoreClient(QObject *parent = nullptr, const QString &program = {}, int timeoutMs = 10000,
                        const QString &socketPath = {});
    ~CoreClient() override;
    bool busy() const { return m_running || !m_queue.isEmpty(); }
    bool offline() const { return m_offline; }

    void day(const QDate &date, Callback callback);
    void stats(const QDate &from, const QDate &to, Callback callback);
    void statsDays(int days, Callback callback);
    void statsFiltered(int days, const QString &tagId, const QString &projectId, const QString &itemId,
                       Callback callback, const QDate &from = {}, const QDate &to = {});
    // stats.get with a ready parameter object and the same filters offline.
    void statsQuery(const QJsonObject &params, Callback callback);
    void statsTable(const QJsonObject &params, Callback callback);
    void statsSeries(const QJsonObject &params, Callback callback);
    // A daemon-only read (stats.table, sessions.list). Reads are coalesced by
    // method plus parameters.
    void rpcRead(const QString &method, const QJsonObject &params, Callback callback);
    void itemsList(Callback callback);
    void itemAdd(const ItemFields &fields, Callback callback = {});
    void itemEdit(const QString &id, const ItemFields &fields, Callback callback = {}, qint64 expectedVersion = -1);
    void itemRemove(const QString &id, Callback callback = {}, qint64 expectedVersion = -1);
    void itemMove(const QString &id, const QString &block, int index, Callback callback = {}, qint64 expectedVersion = -1,
                  const QString &afterId = {}, const QString &beforeId = {});
    void sessionLog(const QString &itemId, const QString &projectId = {}, int minutes = -1,
                    const QJsonObject &values = {}, const QString &note = {},
                    const QDate &date = {}, Callback callback = {}, const QString &epicId = {}, const QString &operation = {});
    void sessionStart(const QString &itemId, const QString &projectId = {}, Callback callback = {});
    void sessionStop(const QString &sessionId = {}, const QJsonObject &values = {},
                     const QString &note = {}, Callback callback = {}, bool done = false, int minutes = -1, const QString &operation = {});
    // session.edit: minutes < 0 keeps them; values/note are sent only when
    // the matching has* flag is set (an empty note clears it).
    void sessionEdit(const QString &sessionId, int minutes, bool hasValues, const QJsonObject &values,
                     bool hasNote, const QString &note, Callback callback = {}, qint64 version = -1,
                     const QString &operation = {});
    void sessionRemove(const QString &sessionId, Callback callback = {});
    void sessionBump(const QString &sessionId, const QString &metricId, int delta, Callback callback = {});
    void sessionNote(const QString &sessionId, const QString &text, Callback callback = {});
    void status(Callback callback);
    void prayerMark(const QString &prayer, const QDate &date, Callback callback = {}, qint64 expectedVersion = -1);
    void prayerUnmark(const QString &prayer, const QDate &date, Callback callback = {}, qint64 expectedVersion = -1);
    void parse(const QString &text, Callback callback);
    // Read-only draft validation/preview; served by the daemon only.
    void preview(const ItemFields &fields, Callback callback);
    void configShow(Callback callback);
    void configSetSleepTime(const QString &time, Callback callback = {});
    void listEntities(const QString &method, Callback callback);
    void mutateEntity(const QString &method, QJsonObject params, Callback callback = {});

    static QString makeOpId();
    QString socketPath() const { return m_socketPath; }

signals:
    void busyChanged();
    void requestFinished(QStringList arguments, bool success);
    void offlineChanged(bool offline);
    void reconnected();
    void changed(qint64 revision, const QStringList &scopes);

private:
    struct Request {
        QString method;
        QJsonObject params;
        QStringList args;
        QList<Callback> callbacks;
        bool read = false;
        bool mutation = false;
        bool expectJson = true;
        bool socketOnly = false;
        QString opId;
        std::function<Dto::AnyDecoded(const QJsonValue &)> decoder;
        std::function<Dto::AnyDecoded(const QJsonValue &)> cliDecoder;
        quint64 generation = 0;
    };
    quint64 m_readGeneration = 0;
    enum class Mode { Waiting, Socket, Cli };

    void configureTransport(const QString &socketPath);
    void enqueue(const QStringList &args, QJsonObject params, const QString &method, Callback callback,
                 bool read, bool mutation, std::function<Dto::AnyDecoded(const QJsonValue &)> decoder,
                 std::function<Dto::AnyDecoded(const QJsonValue &)> cliDecoder, const QString &opId = {},
                 bool socketOnly = false);
    void finishWithError(const QString &message);
    void startNext();
    void startSocket();
    void startProcess();
    void finish(bool timedOut = false);
    void completeRpc(const SocketTransport::Reply &reply);
    using Decoder = std::function<Dto::AnyDecoded(const QJsonValue &)>;
    void writeRpc(const QString &method, const QJsonObject &params, Callback callback, Decoder decoder = Dto::decodeMutationResult);
    static QStringList reportArguments(const QJsonObject &params, QStringList args);
    static QStringList dateArgs(const QDate &date);
    static QString stripErrorPrefix(QString message);

    QString m_program;
    QString m_socketPath;
    int m_timeoutMs;
    Mode m_mode = Mode::Waiting;
    bool m_offline = false;
    QQueue<Request> m_queue;
    Request m_current;
    SocketTransport *m_transport = nullptr;
    QProcess m_process;
    QTimer m_timeout;
    bool m_running = false;
};
