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
    connect(m_transport, &SocketTransport::changed, this, [this](qint64 revision, const QStringList &scopes) {
        ++m_readGeneration;
        emit changed(revision, scopes);
    });
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
                         std::function<Dto::AnyDecoded(const QJsonValue &)> cliDecoder, const QString &opId,
                         bool socketOnly) {
    if (read) {
        const bool queuedMutation = std::any_of(m_queue.cbegin(), m_queue.cend(), [](const Request &r) { return !r.read; });
        if (!queuedMutation && m_running && m_current.read && m_current.generation == m_readGeneration
            && m_current.method == method && m_current.params == params) {
            if (callback) m_current.callbacks.append(std::move(callback));
            return;
        }
        for (auto it = m_queue.end(); it != m_queue.begin();) {
            Request &request = *--it;
            if (!request.read) break;
            if (request.generation == m_readGeneration && request.method == method && request.params == params) {
                if (callback) request.callbacks.append(std::move(callback));
                return;
            }
        }
    }
    Request request{method, std::move(params), args, {}, read, mutation, bool(cliDecoder), socketOnly,
                    opId, std::move(decoder), std::move(cliDecoder), m_readGeneration};
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
    if (m_mode == Mode::Socket) startSocket();
    else if (m_current.socketOnly) { finishWithError(tr("This action needs the Salah daemon.")); return; }
    else startProcess();
    emit busyChanged();
}

void CoreClient::startSocket() {
    m_timeout.stop();
    m_transport->send(m_current.method, m_current.params, [this](const SocketTransport::Reply &reply) {
        completeRpc(reply);
    });
}

void CoreClient::startProcess() {
    m_process.setProgram(m_program);
    m_process.setArguments(m_current.mutation ? QStringList{QStringLiteral("rpc")} : m_current.args);
    m_process.start();
    if (m_current.mutation) {
        auto frame = [](int id, const QString &method, const QJsonObject &params) {
            return QJsonDocument(QJsonObject{{"jsonrpc", "2.0"}, {"id", id}, {"method", method},
                                             {"params", params}}).toJson(QJsonDocument::Compact) + '\n';
        };
        m_process.write(frame(1, QStringLiteral("hello"), SocketTransport::helloParams()));
        m_process.write(frame(2, m_current.method, m_current.params));
        m_process.closeWriteChannel();
    }
    m_timeout.start(m_timeoutMs);
    emit busyChanged();
}

QString CoreClient::stripErrorPrefix(QString message) {
    message = message.trimmed();
    if (message.startsWith("salah: ")) message.remove(0, 7);
    return message;
}

void CoreClient::completeRpc(const SocketTransport::Reply &reply) {
    if (!m_running) return;
    Result result;
    const Request request = std::move(m_current);
    result.opId = request.opId;
    if (!reply.ok) {
        ErrorKind kind = ErrorKind::Rpc;
        if (reply.kind == QStringLiteral("timeout")) kind = ErrorKind::Timeout;
        else if (reply.kind == QStringLiteral("conflict")) kind = ErrorKind::Conflict;
        else if (reply.kind == QStringLiteral("connection_lost") || reply.kind == QStringLiteral("not_ready")) kind = ErrorKind::Process;
        result.error = {kind, reply.message, reply.value.toObject()};
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


void CoreClient::finishWithError(const QString &message) {
    if (!m_running) return;
    const QStringList args = m_current.args;
    const auto callbacks = m_current.callbacks;
    Result result;
    result.opId = m_current.opId;
    result.error = {ErrorKind::Rpc, message};
    m_running = false;
    for (const Callback &callback : callbacks) callback(result);
    emit requestFinished(args, false);
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
    } else if (m_current.mutation) {
        for (const auto &line : m_process.readAllStandardOutput().split('\n')) {
            const auto frame = QJsonDocument::fromJson(line).object();
            if (frame.value("jsonrpc") == "2.0" && frame.value("id").toInt() == 2
                && (frame.contains("error") || frame.contains("result"))) {
                completeRpc(SocketTransport::decodeReply(frame));
                return;
            }
        }
        result.error = {ErrorKind::Decode, tr("Salah returned no matching RPC response.")};
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
    statsFiltered(0, {}, {}, {}, std::move(callback), from, to);
}
void CoreClient::statsDays(int days, Callback callback) {
    statsFiltered(days, {}, {}, {}, std::move(callback));
}
void CoreClient::statsFiltered(int days, const QString &tagId, const QString &projectId, const QString &itemId,
                               Callback callback, const QDate &from, const QDate &to) {
    QJsonObject params;
    if (from.isValid() && to.isValid()) {
        const QString first = from.toString(Qt::ISODate), last = to.toString(Qt::ISODate);
        params = {{"from", first}, {"to", last}};
    } else {
        params.insert(QStringLiteral("days"), days);
    }
    if (!tagId.isEmpty()) params.insert("tag_id", tagId);
    if (!projectId.isEmpty()) params.insert("project_id", projectId);
    if (!itemId.isEmpty()) params.insert("item_id", itemId);
    statsQuery(params, std::move(callback));
}
void CoreClient::statsQuery(const QJsonObject &params, Callback callback) {
    auto args = reportArguments(params, {QStringLiteral("stats")});
    args << QStringLiteral("--json");
    enqueue(args, params, QStringLiteral("stats.get"), std::move(callback), true, false,
            Dto::decodeStats, Dto::decodeStats);
}

QStringList CoreClient::reportArguments(const QJsonObject &params, QStringList args) {
    auto option = [&](const char *key, const char *flag) {
        const QJsonValue value = params.value(QLatin1String(key));
        if (!value.isUndefined()) args << QLatin1String(flag) << value.toVariant().toString();
    };
    option("days", "--days");
    option("from", "--from");
    option("to", "--to");
    option("tag_id", "--tag");
    option("project_id", "--project");
    option("epic_id", "--epic");
    option("item_id", "--item");
    option("minutes_min", "--min-minutes");
    option("minutes_max", "--max-minutes");
    option("group", "--group");
    option("interval", "--interval");
    option("split", "--split");
    for (const auto &measure : params.value(QStringLiteral("measures")).toArray())
        args << QStringLiteral("--measure") << measure.toString();
    const auto values = params.value(QStringLiteral("values")).toObject();
    for (auto metric = values.begin(); metric != values.end(); ++metric) {
        const auto bounds = metric.value().toObject();
        for (auto bound = bounds.begin(); bound != bounds.end(); ++bound)
            args << (QStringLiteral("--") + bound.key())
                 << (metric.key() + QLatin1Char('=') + QString::number(bound.value().toDouble(), 'g', 17));
    }
    return args;
}

void CoreClient::statsSeries(const QJsonObject &params, Callback callback) {
    enqueue(reportArguments(params, {"stats", "series"}), params, QStringLiteral("stats.series"),
            std::move(callback), true, false, Dto::decodeJson, Dto::decodeJson);
}

void CoreClient::statsTable(const QJsonObject &params, Callback callback) {
    enqueue(reportArguments(params, {"stats", "table"}), params, QStringLiteral("stats.table"),
            std::move(callback), true, false, Dto::decodeJson, Dto::decodeJson);
}

void CoreClient::rpcRead(const QString &method, const QJsonObject &params, Callback callback) {
    enqueue({QStringLiteral("rpc"), method}, params, method, std::move(callback), true, false,
            Dto::decodeJson, nullptr, {}, true);
}
void CoreClient::itemsList(Callback callback) {
    enqueue({"item", "list", "--json"}, {}, QStringLiteral("items.list"), std::move(callback), true, false,
            Dto::decodeJson, Dto::decodeJson);
}
void CoreClient::itemAdd(const ItemFields &f, Callback callback) {
    const QString opId = f.opId.isEmpty() ? makeOpId() : f.opId;
    QJsonObject params{{"op_id", opId}};
    if (f.quick) params.insert(QStringLiteral("quick_text"), f.title);
    else params.insert(QStringLiteral("title"), f.title);
    if (f.backlog) params.insert(QStringLiteral("backlog"), true);
    else if (!f.block.isEmpty()) params.insert(QStringLiteral("block"), f.block);
    if (!f.quick) {
        params.insert(QStringLiteral("repeat"), f.repeat);
        if (!f.weekdays.isEmpty()) {
            QJsonArray weekdays;
            for (const QString &day : f.weekdays) weekdays.append(day);
            params.insert(QStringLiteral("weekdays"), weekdays);
        }
        if (!f.projectId.isEmpty()) params.insert(QStringLiteral("project_id"), f.projectId);
        if (f.epicId.isString()) params.insert(QStringLiteral("epic_id"), f.epicId);
        if (f.hasPlannedMinutes) params.insert(QStringLiteral("planned_minutes"), f.plannedMinutes);
        if (!f.metricIds.isUndefined()) params.insert(QStringLiteral("metric_ids"), f.metricIds);
    }
    if (!f.tagIds.isUndefined()) params.insert(QStringLiteral("tag_ids"), f.tagIds);
    if (!f.backlog && f.date.isValid()) params.insert(QStringLiteral("date"), f.date.toString(Qt::ISODate));
    writeRpc(QStringLiteral("item.add"), params, std::move(callback));
}
void CoreClient::itemEdit(const QString &id, const ItemFields &f, Callback callback, qint64 expectedVersion) {
    const QString opId = makeOpId();
    QJsonObject params{{"op_id", opId}, {"id", id}, {"title", f.title}, {"repeat", f.repeat}};
    if (!f.backlog) params.insert(QStringLiteral("block"), f.block);
    if (!f.weekdays.isEmpty()) {
        QJsonArray weekdays;
        for (const QString &day : f.weekdays) weekdays.append(day);
        params.insert(QStringLiteral("weekdays"), weekdays);
    }
    params.insert(QStringLiteral("project_id"), f.projectId.isEmpty() ? QJsonValue(QJsonValue::Null) : QJsonValue(f.projectId));
    params.insert(QStringLiteral("planned_minutes"), f.hasPlannedMinutes ? QJsonValue(f.plannedMinutes) : QJsonValue(QJsonValue::Null));
    if (!f.metricIds.isUndefined()) params.insert(QStringLiteral("metric_ids"), f.metricIds);
    if (!f.tagIds.isUndefined()) params.insert(QStringLiteral("tag_ids"), f.tagIds);
    if (!f.epicId.isUndefined()) params.insert(QStringLiteral("epic_id"), f.epicId);
    if (expectedVersion >= 0) params.insert(QStringLiteral("expected_version"), expectedVersion);
    writeRpc(QStringLiteral("item.edit"), params, std::move(callback));
}
void CoreClient::itemRemove(const QString &id, Callback callback, qint64 expectedVersion) {
    const QString opId = makeOpId();
    QJsonObject params{{"op_id", opId}, {"id", id}};
    if (expectedVersion >= 0) params.insert(QStringLiteral("expected_version"), expectedVersion);
    writeRpc(QStringLiteral("item.remove"), params, std::move(callback));
}
void CoreClient::itemMove(const QString &id, const QString &block, int index, Callback callback, qint64 expectedVersion,
                          const QString &afterId, const QString &beforeId) {
    const QString opId = makeOpId();
    QJsonObject params{{"op_id", opId}, {"id", id},
                       {"block", block.isEmpty() ? QJsonValue(QJsonValue::Null) : QJsonValue(block)}};
    // A sibling anchor resolves inside the write transaction.
    if (!afterId.isEmpty()) params.insert(QStringLiteral("after_id"), afterId);
    else if (!beforeId.isEmpty()) params.insert(QStringLiteral("before_id"), beforeId);
    else params.insert(QStringLiteral("index"), index);
    if (expectedVersion >= 0) params.insert(QStringLiteral("expected_version"), expectedVersion);
    writeRpc(QStringLiteral("item.move"), params, std::move(callback));
}
void CoreClient::sessionLog(const QString &id, const QString &projectId, int minutes,
                            const QJsonObject &values, const QString &note, const QDate &date,
                            Callback callback, const QString &epicId, const QString &operation) {
    const QString opId = operation.isEmpty() ? makeOpId() : operation;
    QJsonObject params{{"op_id", opId}};
    if (!id.isEmpty()) params.insert(QStringLiteral("item_id"), id);
    if (!projectId.isEmpty()) params.insert(QStringLiteral("project_id"), projectId);
    if (!epicId.isEmpty()) params.insert(QStringLiteral("epic_id"), epicId);
    if (minutes >= 0) params.insert(QStringLiteral("minutes"), minutes);
    if (!values.isEmpty()) params.insert(QStringLiteral("values"), values);
    if (!note.isEmpty()) params.insert(QStringLiteral("note"), note);
    if (date.isValid()) params.insert(QStringLiteral("date"), date.toString(Qt::ISODate));
    writeRpc(QStringLiteral("session.log"), params, std::move(callback));
}
void CoreClient::sessionStart(const QString &id, const QString &projectId, Callback callback) {
    const QString opId = makeOpId();
    QJsonObject params{{"op_id", opId}};
    if (!id.isEmpty()) params.insert(QStringLiteral("item_id"), id);
    if (!projectId.isEmpty()) params.insert(QStringLiteral("project_id"), projectId);
    writeRpc(QStringLiteral("session.start"), params, std::move(callback));
}
void CoreClient::sessionStop(const QString &id, const QJsonObject &values, const QString &note,
                             Callback callback, bool done, int minutes, const QString &operation) {
    const QString opId = operation.isEmpty() ? makeOpId() : operation;
    QJsonObject params{{"op_id", opId}};
    if (!id.isEmpty()) params.insert(QStringLiteral("id"), id);
    if (!values.isEmpty()) params.insert(QStringLiteral("values"), values);
    if (!note.isEmpty()) params.insert(QStringLiteral("note"), note);
    if (done) params.insert(QStringLiteral("done"), true);
    if (minutes >= 0) params.insert(QStringLiteral("minutes"), minutes);

    writeRpc(QStringLiteral("session.stop"), params, std::move(callback));
}
void CoreClient::sessionEdit(const QString &id, int minutes, bool hasValues, const QJsonObject &values,
                             bool hasNote, const QString &note, Callback callback, qint64 version, const QString &operation) {
    const QString opId = operation.isEmpty() ? makeOpId() : operation;
    QJsonObject params{{"op_id", opId}, {"id", id}};
    if (version >= 0) params.insert(QStringLiteral("expected_version"), version);
    if (minutes >= 0) params.insert(QStringLiteral("minutes"), minutes);
    if (hasValues) params.insert(QStringLiteral("values"), values);
    if (hasNote)
        params.insert(QStringLiteral("note"), note.trimmed().isEmpty() ? QJsonValue(QJsonValue::Null) : QJsonValue(note));
    writeRpc(QStringLiteral("session.edit"), params, std::move(callback));
}
void CoreClient::sessionRemove(const QString &id, Callback callback) {
    const QString opId = makeOpId();
    writeRpc(QStringLiteral("session.remove"), QJsonObject{{"id", id}, {"op_id", opId}}, std::move(callback));
}
void CoreClient::sessionBump(const QString &id, const QString &metricId, int delta, Callback callback) {
    const QString opId = makeOpId();
    QJsonObject params{{"op_id", opId}, {"metric_id", metricId}, {"delta", delta}};
    if (!id.isEmpty()) params.insert(QStringLiteral("id"), id);
    writeRpc(QStringLiteral("session.bump"), params, std::move(callback));
}

void CoreClient::sessionNote(const QString &id, const QString &text, Callback callback) {
    const QString opId = makeOpId();
    QJsonObject params{{"op_id", opId}, {"text", text.trimmed()}};
    if (!id.isEmpty()) params.insert(QStringLiteral("id"), id);
    writeRpc(QStringLiteral("session.note"), params, std::move(callback));
}

void CoreClient::status(Callback callback) {
    enqueue({"status", "--json"}, QJsonObject{}, QStringLiteral("status.get"), std::move(callback), true, false,
            Dto::decodeJson, Dto::decodeJson);
}
void CoreClient::prayerMark(const QString &prayer, const QDate &date, Callback callback, qint64 expectedVersion) {
    const QString opId = makeOpId();
    QJsonObject params{{"op_id", opId}, {"prayer", prayer}};
    if (date.isValid()) params.insert(QStringLiteral("date"), date.toString(Qt::ISODate));
    if (expectedVersion >= 0) params.insert(QStringLiteral("expected_version"), expectedVersion);
    writeRpc(QStringLiteral("prayer.mark"), params, std::move(callback));
}
void CoreClient::prayerUnmark(const QString &prayer, const QDate &date, Callback callback, qint64 expectedVersion) {
    const QString opId = makeOpId();
    QJsonObject params{{"op_id", opId}, {"prayer", prayer}};
    if (date.isValid()) params.insert(QStringLiteral("date"), date.toString(Qt::ISODate));
    if (expectedVersion >= 0) params.insert(QStringLiteral("expected_version"), expectedVersion);
    writeRpc(QStringLiteral("prayer.unmark"), params, std::move(callback));
}
void CoreClient::parse(const QString &text, Callback callback) {
    enqueue({"item","parse","--json","--",text}, QJsonObject{{"text", text}}, QStringLiteral("item.parse"),
            std::move(callback), true, false, Dto::decodeParseDraft, Dto::decodeParseDraft);
}
void CoreClient::preview(const ItemFields &f, Callback callback) {
    const QString date = f.date.isValid() ? f.date.toString(Qt::ISODate) : QString();
    QJsonObject params{{"block", f.block}, {"repeat", f.repeat}, {"date", date}};
    QStringList args{"item", "preview", "--json", "--block", f.block, "--repeat", f.repeat,
                     "--date", date};
    if (!f.weekdays.isEmpty()) {
        QJsonArray weekdays;
        for (const QString &day : f.weekdays) weekdays.append(day);
        params.insert(QStringLiteral("weekdays"), weekdays);
        args << "--days" << f.weekdays.join(',');
    }
    if (!f.projectId.isEmpty()) params.insert(QStringLiteral("project_id"), f.projectId);
    if (f.hasPlannedMinutes) params.insert(QStringLiteral("planned_minutes"), f.plannedMinutes);
    enqueue(args, params, QStringLiteral("item.preview"), std::move(callback), true, false,
            Dto::decodePreview, nullptr, {}, true);
}
void CoreClient::configShow(Callback callback) {
    enqueue({"config","show"}, {}, QStringLiteral("config.get"), std::move(callback), true, false,
            Dto::decodeConfig, Dto::decodeConfig);
}
void CoreClient::configSetSleepTime(const QString &time, Callback callback) {
    const QString opId = makeOpId();
    writeRpc(QStringLiteral("config.set"), QJsonObject{{"op_id", opId}, {"sleep_time", time}}, std::move(callback), Dto::decodeConfig);
}
void CoreClient::listEntities(const QString &method, Callback callback) {
    // No archive filter: Settings shows archived rows to restore.
    enqueue({QStringLiteral("rpc"), method}, QJsonObject{}, method, std::move(callback), true, false,
            Dto::decodeJson, nullptr, {}, true);
}
void CoreClient::mutateEntity(const QString &method, QJsonObject params, Callback callback) {
    const QString opId = makeOpId();
    params.insert(QStringLiteral("op_id"), opId);
    writeRpc(method, params, std::move(callback));
}

void CoreClient::writeRpc(const QString &method, const QJsonObject &params, Callback callback,
                          Decoder decoder) {
    enqueue({QStringLiteral("rpc"), method}, params, method, std::move(callback), false, true,
            decoder, nullptr, params.value(QStringLiteral("op_id")).toString());
}
