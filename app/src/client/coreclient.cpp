#include "coreclient.h"

#include <QDateTime>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonParseError>
#include <QUuid>
#include <algorithm>

CoreClient::CoreClient(QObject *parent, const QString &program, int timeoutMs, const QString &socketPath)
    : QObject(parent), m_program(program.isEmpty() ? qEnvironmentVariable("SALAH_BIN", "salah") : program),
      m_timeoutMs(timeoutMs) {
    QString path = socketPath;
    if (path.isEmpty()) {
        path = qEnvironmentVariable("SALAH_SOCKET");
        // Only the app (no explicit CLI program) probes the canonical socket.
        // Tests and other callers that pass a program own their transport;
        // silently switching to the user's real daemon would make them depend
        // on the desktop session and on whatever state that daemon holds.
        if (path.isEmpty() && program.isEmpty()) path = SocketTransport::defaultSocketPath();
    }
    if (!path.isEmpty() && !QFileInfo::exists(path)) {
        // No socket at all: use the CLI immediately instead of waiting for a
        // failed connect (and never hold a request in the Waiting state).
        path.clear();
    }
    m_timeout.setSingleShot(true);
    connect(&m_timeout, &QTimer::timeout, this, [this] { finish(true); });
    connect(&m_process, &QProcess::finished, this, [this](int, QProcess::ExitStatus) {
        if (m_running) finish();
    });
    connect(&m_process, &QProcess::errorOccurred, this, [this](QProcess::ProcessError error) {
        if (m_running && error == QProcess::FailedToStart) finish();
    });
    configureTransport(path);
}

CoreClient::~CoreClient() {
    m_timeout.stop();
    m_queue.clear();
    m_running = false;
    m_process.disconnect(this);
    if (m_process.state() != QProcess::NotRunning) {
        m_process.kill();
        m_process.waitForFinished(500);
    }
}

void CoreClient::configureTransport(const QString &path) {
    if (path.isEmpty()) {
        m_mode = Mode::Cli;
        m_offline = true;
        return;
    }
    m_socketPath = path;
    m_transport = new SocketTransport(this);
    m_transport->setSocketPath(path);
    m_transport->setTimeoutMs(m_timeoutMs);
    connect(m_transport, &SocketTransport::readyChanged, this, [this](bool ready) {
        if (ready) {
            m_mode = Mode::Socket;
            if (m_offline) {
                m_offline = false;
                emit offlineChanged(false);
            }
            startNext();
        } else if (m_mode == Mode::Socket) {
            m_mode = Mode::Waiting;
        }
    });
    connect(m_transport, &SocketTransport::fellBackToCli, this, [this] {
        m_mode = Mode::Cli;
        if (!m_offline) {
            m_offline = true;
            emit offlineChanged(true);
        }
        startNext();
    });
    connect(m_transport, &SocketTransport::reconnected, this, &CoreClient::reconnected);
    connect(m_transport, &SocketTransport::changed, this, &CoreClient::changed);
    m_transport->start();
}

QString CoreClient::makeOpId() {
    return QUuid::createUuid().toString(QUuid::WithoutBraces);
}

QStringList CoreClient::dateArgs(const QDate &date) {
    return date.isValid() ? QStringList{"--date", date.toString(Qt::ISODate)} : QStringList{};
}

void CoreClient::enqueue(const QStringList &args, QJsonObject params, const QString &method, Callback callback,
                         bool read, bool mutation, std::function<Dto::AnyDecoded(const QJsonValue &)> decoder,
                         std::function<Dto::AnyDecoded(const QJsonValue &)> cliDecoder, const QString &opId) {
    if (read) {
        const bool queuedMutation = std::any_of(m_queue.cbegin(), m_queue.cend(), [](const Request &r) { return !r.read; });
        if (!queuedMutation && m_running && m_current.read && m_current.args == args) {
            if (callback) m_current.callbacks.append(std::move(callback));
            return;
        }
        for (auto it = m_queue.end(); it != m_queue.begin();) {
            Request &request = *--it;
            if (!request.read) break;
            if (request.read && request.args == args) {
                if (callback) request.callbacks.append(std::move(callback));
                return;
            }
        }
    }
    Request request{method, std::move(params), args, {}, read, mutation, bool(cliDecoder), opId,
                    std::move(decoder), std::move(cliDecoder)};
    if (callback) request.callbacks.append(std::move(callback));
    m_queue.enqueue(std::move(request));
    emit busyChanged();
    startNext();
}

void CoreClient::startNext() {
    if (m_running || m_queue.isEmpty()) return;
    if (m_mode == Mode::Waiting) return;
    if (m_mode == Mode::Socket && (!m_transport || !m_transport->ready())) {
        m_mode = Mode::Waiting;
        return;
    }
    m_current = m_queue.dequeue();
    m_running = true;
    if (m_mode == Mode::Socket) startSocket(m_current);
    else startProcess(m_current);
    emit busyChanged();
}

void CoreClient::startSocket(Request request) {
    m_timeout.stop();
    if (!m_transport) {
        // Defensive: a request can only be dispatched in Socket mode, but never
        // dereference the transport if that invariant is broken (for example
        // during teardown or a failed configuration).
        completeSocket(request, SocketTransport::Reply{false, false, QStringLiteral("not_ready"),
                                                       tr("Salah daemon is not reachable."), {}});
        return;
    }
    const QString method = request.method;
    const QJsonObject params = request.params;
    m_transport->send(method, params,
                      [this, request = std::move(request)](const SocketTransport::Reply &reply) mutable {
                          completeSocket(request, reply);
                      });
}

void CoreClient::startProcess(Request request) {
    m_process.setProgram(m_program);
    m_process.setArguments(request.args);
    m_process.start();
    m_timeout.start(m_timeoutMs);
    emit busyChanged();
}

QString CoreClient::stripErrorPrefix(QString message) {
    message = message.trimmed();
    if (message.startsWith("salah: ")) message.remove(0, 7);
    return message;
}

void CoreClient::completeSocket(const Request &request, const SocketTransport::Reply &reply) {
    if (!m_running) return;
    Result result;
    result.opId = request.opId;
    if (!reply.ok) {
        ErrorKind kind = ErrorKind::Rpc;
        if (reply.kind == QStringLiteral("timeout")) kind = ErrorKind::Timeout;
        else if (reply.kind == QStringLiteral("conflict")) kind = ErrorKind::Conflict;
        else if (reply.kind == QStringLiteral("connection_lost") || reply.kind == QStringLiteral("not_ready")) kind = ErrorKind::Process;
        result.error = {kind, reply.message};
    } else if (request.mutation && !request.decoder) {
        result.ok = true;
    } else {
        const Dto::AnyDecoded decoded = request.decoder ? request.decoder(reply.value) : Dto::AnyDecoded{};
        if (!decoded) {
            const QString detail = decoded.error ? decoded.error->formatted() : QStringLiteral("missing decoder");
            result.error = {ErrorKind::Decode, tr("Salah returned invalid data: %1").arg(detail)};
        } else {
            result.ok = true;
            result.value = *decoded.value;
        }
    }
    const QStringList args = request.args;
    const auto callbacks = request.callbacks;
    m_running = false;
    for (const Callback &callback : callbacks) callback(result);
    emit requestFinished(args, result.ok);
    emit busyChanged();
    startNext();
}

void CoreClient::finish(bool timedOut) {
    if (!m_running) return;
    m_timeout.stop();
    Result result;
    result.opId = m_current.opId;
    if (timedOut) {
        result.error = {ErrorKind::Timeout, tr("Salah command timed out.")};
        m_running = false;
        m_process.kill();
        m_process.waitForFinished(500);
    } else if (m_process.error() == QProcess::FailedToStart ||
               m_process.exitStatus() != QProcess::NormalExit || m_process.exitCode() != 0) {
        QString detail = stripErrorPrefix(QString::fromUtf8(m_process.readAllStandardError()));
        if (detail.isEmpty()) detail = tr("Could not run Salah: %1").arg(m_process.errorString());
        result.error = {m_process.error() == QProcess::FailedToStart ? ErrorKind::Process : ErrorKind::Exit, detail};
    } else if (!m_current.expectJson) {
        result.ok = true;
    } else {
        QJsonParseError parseError;
        const QJsonDocument document = QJsonDocument::fromJson(m_process.readAllStandardOutput(), &parseError);
        if (parseError.error != QJsonParseError::NoError || (!document.isObject() && !document.isArray())) {
            const QString detail = parseError.error == QJsonParseError::NoError
                ? tr("JSON root is not an object or array") : parseError.errorString();
            result.error = {ErrorKind::Decode, tr("Salah returned invalid JSON: %1").arg(detail)};
        } else {
            const QJsonValue root = document.isObject() ? QJsonValue(document.object()) : QJsonValue(document.array());
            const Dto::AnyDecoded decoded = m_current.cliDecoder ? m_current.cliDecoder(root) : Dto::AnyDecoded{};
            if (!decoded) {
                const QString detail = decoded.error ? decoded.error->formatted() : QStringLiteral("missing decoder");
                result.error = {ErrorKind::Decode, tr("Salah returned invalid data: %1").arg(detail)};
            } else {
                result.ok = true;
                result.value = *decoded.value;
            }
        }
    }
    const QStringList args = m_current.args;
    const auto callbacks = m_current.callbacks;
    m_running = false;
    for (const Callback &callback : callbacks) callback(result);
    emit requestFinished(args, result.ok);
    emit busyChanged();
    startNext();
}

void CoreClient::day(const QDate &date, Callback callback) {
    QStringList args{"day"}; args << dateArgs(date) << "--json";
    QJsonObject params;
    if (date.isValid()) params.insert(QStringLiteral("date"), date.toString(Qt::ISODate));
    enqueue(args, params, QStringLiteral("day.get"), std::move(callback), true, false,
            Dto::decodeDayPlan, Dto::decodeDayPlan);
}
void CoreClient::stats(const QDate &from, const QDate &to, Callback callback) {
    QJsonObject params{{"from", from.toString(Qt::ISODate)}, {"to", to.toString(Qt::ISODate)}};
    enqueue({"stats", "--from", from.toString(Qt::ISODate), "--to", to.toString(Qt::ISODate), "--json"},
            params, QStringLiteral("stats.get"), std::move(callback), true, false,
            Dto::decodeStats, Dto::decodeStats);
}
void CoreClient::statsDays(int days, Callback callback) {
    QJsonObject params{{"days", days}};
    enqueue({"stats", "--days", QString::number(days), "--json"}, params, QStringLiteral("stats.get"),
            std::move(callback), true, false, Dto::decodeStats, Dto::decodeStats);
}
void CoreClient::itemAdd(const ItemFields &f, Callback callback) {
    QStringList args{"item", "add", "--json"};
    if (f.quick) args << "--quick";
    if (!f.block.isEmpty()) args << "--block" << f.block;
    if (!f.quick) {
        args << "--repeat" << f.repeat;
        if (!f.weekdays.isEmpty()) args << "--days" << f.weekdays.join(',');
        args << "--measure" << f.measure;
        if (f.hasTarget) args << "--target" << QString::number(f.target);
    }
    args << dateArgs(f.date) << "--" << f.title;
    const QString opId = f.opId.isEmpty() ? makeOpId() : f.opId;
    QJsonObject params{{"op_id", opId}};
    if (f.quick) params.insert(QStringLiteral("quick_text"), f.title);
    else params.insert(QStringLiteral("title"), f.title);
    if (!f.block.isEmpty()) params.insert(QStringLiteral("block"), f.block);
    if (!f.quick) {
        params.insert(QStringLiteral("repeat"), f.repeat);
        if (!f.weekdays.isEmpty()) {
            QJsonArray weekdays;
            for (const QString &day : f.weekdays) weekdays.append(day);
            params.insert(QStringLiteral("weekdays"), weekdays);
        }
        params.insert(QStringLiteral("measure"), f.measure);
        if (f.hasTarget) params.insert(QStringLiteral("target"), f.target);
    }
    if (f.date.isValid()) params.insert(QStringLiteral("date"), f.date.toString(Qt::ISODate));
    enqueue(args, params, QStringLiteral("item.add"), std::move(callback), false, true,
            Dto::decodeMutationResult, Dto::decodeItem, opId);
}
void CoreClient::itemEdit(const QString &id, const ItemFields &f, Callback callback, qint64 expectedVersion) {
    QStringList args{"item", "edit", "--json", id, "--title=" + f.title, "--block", f.block, "--repeat", f.repeat};
    if (!f.weekdays.isEmpty()) args << "--days" << f.weekdays.join(',');
    args << "--measure" << f.measure;
    if (f.hasTarget) args << "--target" << QString::number(f.target); else args << "--no-target";
    const QString opId = makeOpId();
    QJsonObject params{{"op_id", opId}, {"id", id}, {"title", f.title}, {"block", f.block},
                       {"repeat", f.repeat}, {"measure", f.measure}};
    if (!f.weekdays.isEmpty()) {
        QJsonArray weekdays;
        for (const QString &day : f.weekdays) weekdays.append(day);
        params.insert(QStringLiteral("weekdays"), weekdays);
    }
    params.insert(QStringLiteral("target"), f.hasTarget ? QJsonValue(f.target) : QJsonValue(QJsonValue::Null));
    if (expectedVersion >= 0) params.insert(QStringLiteral("expected_version"), expectedVersion);
    enqueue(args, params, QStringLiteral("item.edit"), std::move(callback), false, true,
            Dto::decodeMutationResult, Dto::decodeItem, opId);
}
void CoreClient::itemRemove(const QString &id, Callback callback, qint64 expectedVersion) {
    const QString opId = makeOpId();
    QJsonObject params{{"op_id", opId}, {"id", id}};
    if (expectedVersion >= 0) params.insert(QStringLiteral("expected_version"), expectedVersion);
    enqueue({"item", "remove", "--json", id}, params, QStringLiteral("item.remove"), std::move(callback),
            false, true, Dto::decodeMutationResult, Dto::decodeItem, opId);
}
void CoreClient::itemMove(const QString &id, const QString &block, int index, Callback callback, qint64 expectedVersion) {
    const QString opId = makeOpId();
    QJsonObject params{{"op_id", opId}, {"id", id}, {"block", block}, {"index", index}};
    if (expectedVersion >= 0) params.insert(QStringLiteral("expected_version"), expectedVersion);
    enqueue({"item", "move", "--json", id, "--block", block, "--index", QString::number(index)},
            params, QStringLiteral("item.move"), std::move(callback), false, true,
            Dto::decodeMutationResult, Dto::decodeItem, opId);
}
void CoreClient::itemDone(const QString &id, const QDate &date, Callback callback) {
    QStringList a{"item","done","--json",id}; a << dateArgs(date);
    const QString opId = makeOpId();
    QJsonObject params{{"op_id", opId}, {"item_id", id}};
    if (date.isValid()) params.insert(QStringLiteral("date"), date.toString(Qt::ISODate));
    enqueue(a, params, QStringLiteral("entry.done"), std::move(callback), false, true,
            Dto::decodeMutationResult, Dto::decodeEntry, opId);
}
void CoreClient::itemUndo(const QString &id, const QDate &date, Callback callback) {
    QStringList a{"item","undo","--json",id}; a << dateArgs(date);
    const QString opId = makeOpId();
    QJsonObject params{{"op_id", opId}, {"item_id", id}};
    if (date.isValid()) params.insert(QStringLiteral("date"), date.toString(Qt::ISODate));
    enqueue(a, params, QStringLiteral("entry.undo"), std::move(callback), false, true,
            Dto::decodeMutationResult, Dto::decodeUndo, opId);
}
void CoreClient::itemLog(const QString &id, int value, bool minutes, const QDate &date, Callback callback) {
    QStringList a{"item","log","--json"}; if(minutes)a<<"--minutes"; a<<id<<QString::number(value); a<<dateArgs(date);
    const QString opId = makeOpId();
    QJsonObject params{{"op_id", opId}, {"item_id", id}, {"value", value},
                       {"kind", minutes ? QStringLiteral("minutes") : QStringLiteral("count")}};
    if (date.isValid()) params.insert(QStringLiteral("date"), date.toString(Qt::ISODate));
    enqueue(a, params, QStringLiteral("entry.log"), std::move(callback), false, true,
            Dto::decodeMutationResult, minutes ? Dto::decodeEntry : Dto::decodeEntry, opId);
}
void CoreClient::timerStart(const QString &id, Callback callback) {
    const QString opId = makeOpId();
    enqueue({"item","start","--json",id}, QJsonObject{{"op_id", opId}, {"item_id", id}},
            QStringLiteral("timer.start"), std::move(callback), false, true,
            Dto::decodeMutationResult, Dto::decodeEntry, opId);
}
void CoreClient::timerStop(const QString &id, Callback callback) {
    const QString opId = makeOpId();
    QJsonObject params{{"op_id", opId}};
    if (!id.isEmpty()) params.insert(QStringLiteral("item_id"), id);
    enqueue({"item","stop","--json",id}, params, QStringLiteral("timer.stop"), std::move(callback),
            false, true, Dto::decodeMutationResult, Dto::decodeEntries, opId);
}
void CoreClient::prayerMark(const QString &prayer, const QDate &date, Callback callback, qint64 expectedVersion) {
    QStringList a{"prayer","mark","--json",prayer}; a<<dateArgs(date);
    const QString opId = makeOpId();
    QJsonObject params{{"op_id", opId}, {"prayer", prayer}};
    if (date.isValid()) params.insert(QStringLiteral("date"), date.toString(Qt::ISODate));
    if (expectedVersion >= 0) params.insert(QStringLiteral("expected_version"), expectedVersion);
    enqueue(a, params, QStringLiteral("prayer.mark"), std::move(callback), false, true,
            Dto::decodeMutationResult, Dto::decodePrayerMark, opId);
}
void CoreClient::prayerUnmark(const QString &prayer, const QDate &date, Callback callback, qint64 expectedVersion) {
    QStringList a{"prayer","unmark","--json",prayer}; a<<dateArgs(date);
    const QString opId = makeOpId();
    QJsonObject params{{"op_id", opId}, {"prayer", prayer}};
    if (date.isValid()) params.insert(QStringLiteral("date"), date.toString(Qt::ISODate));
    if (expectedVersion >= 0) params.insert(QStringLiteral("expected_version"), expectedVersion);
    enqueue(a, params, QStringLiteral("prayer.unmark"), std::move(callback), false, true,
            Dto::decodeMutationResult, Dto::decodePrayerMark, opId);
}
void CoreClient::parse(const QString &text, Callback callback) {
    enqueue({"item","parse","--json","--",text}, QJsonObject{{"text", text}}, QStringLiteral("item.parse"),
            std::move(callback), true, false, Dto::decodeParseDraft, Dto::decodeParseDraft);
}
void CoreClient::configShow(Callback callback) {
    enqueue({"config","show"}, {}, QStringLiteral("config.get"), std::move(callback), true, false,
            Dto::decodeConfig, Dto::decodeConfig);
}
void CoreClient::configSetSleepTime(const QString &time, Callback callback) {
    const QString opId = makeOpId();
    enqueue({"config","set","--sleep-time",time},
            QJsonObject{{"op_id", opId}, {"sleep_time", time}}, QStringLiteral("config.set"),
            std::move(callback), false, true, Dto::decodeConfig, nullptr, opId);
}
