#include "sockettransport.h"

#include <QDir>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonParseError>
#include <cstdlib>

namespace {
constexpr int kMaxLineBytes = 1024 * 1024;
}

SocketTransport::SocketTransport(QObject *parent) : QObject(parent), m_socket(new QLocalSocket(this)) {
    m_reconnectTimer.setSingleShot(true);
    connect(&m_reconnectTimer, &QTimer::timeout, this, &SocketTransport::beginConnect);
    m_handshakeTimer.setSingleShot(true);
    connect(&m_handshakeTimer, &QTimer::timeout, this, [this] {
        handshakeFailed(tr("Salah daemon did not complete the protocol handshake."));
    });
    connect(m_socket, &QLocalSocket::connected, this, [this] {
        if (m_state != State::Connecting) return;
        m_state = State::Hello;
        m_handshakeId = m_nextId++;
        QJsonObject params{{"protocol", QJsonArray{1}},
                           {"client", QStringLiteral("salah-app/0.4")},
                           {"actor", QStringLiteral("app")}};
        writeRequest(m_handshakeId, QStringLiteral("hello"), params);
        m_handshakeTimer.start(m_timeoutMs);
    });
    connect(m_socket, &QLocalSocket::disconnected, this, [this] {
        if (m_suppressDisconnect) return;
        if (m_state == State::Ready || m_state == State::Hello || m_state == State::Subscribe ||
            m_state == State::Connecting) {
            handleSocketFailure();
        }
    });
    connect(m_socket, &QLocalSocket::errorOccurred, this, [this](QLocalSocket::LocalSocketError error) {
        if (m_state == State::Reconnecting || m_state == State::Idle || m_state == State::Failed) return;
        handleSocketFailure(error);
    });
    connect(m_socket, &QLocalSocket::readyRead, this, &SocketTransport::onReadyRead);
}

SocketTransport::~SocketTransport() {
    m_suppressDisconnect = true;
    m_handshakeTimer.stop();
    m_reconnectTimer.stop();
    m_socket->abort();
}

QString SocketTransport::defaultSocketPath() {
    const QByteArray runtime = qgetenv("XDG_RUNTIME_DIR");
    if (runtime.isEmpty()) return {};
    return QDir(QString::fromLocal8Bit(runtime)).filePath(QStringLiteral("salah/salah.sock"));
}

void SocketTransport::start() {
    if (m_state != State::Idle) return;
    if (m_path.isEmpty() || !QFileInfo::exists(m_path)) {
        // Nothing to connect to: fall back to the CLI at once and quietly.
        failToCli();
        return;
    }
    beginConnect();
}

void SocketTransport::stop() {
    m_suppressDisconnect = true;
    m_handshakeTimer.stop();
    m_reconnectTimer.stop();
    m_socket->abort();
    m_suppressDisconnect = false;
    m_state = State::Idle;
}

void SocketTransport::beginConnect() {
    if (m_state == State::Failed) return;
    if (!QFileInfo::exists(m_path)) {
        failToCli();
        return;
    }
    m_state = State::Connecting;
    m_handshakeTimer.stop();
    m_buffer.clear();
    m_suppressDisconnect = true;
    m_socket->abort();
    m_suppressDisconnect = false;
    m_socket->connectToServer(m_path);
}

void SocketTransport::scheduleReconnect() {
    if (m_state == State::Failed || m_reconnectTimer.isActive()) return;
    m_state = State::Reconnecting;
    m_reconnectTimer.start(m_backoffMs);
    m_backoffMs = qMin(m_backoffMs * 2, 5000);
}

void SocketTransport::handleSocketFailure(QLocalSocket::LocalSocketError error) {
    clearHandshakeTimer();
    if (m_everReady) {
        handleDrop();
        return;
    }
    const bool noListener = error == QLocalSocket::ConnectionRefusedError ||
                            error == QLocalSocket::ServerNotFoundError ||
                            error == QLocalSocket::SocketAccessError ||
                            !QFileInfo::exists(m_path);
    if (noListener) {
        // A missing path or a refused connection means there is no
        // daemon (and no systemd activation pending): go to the CLI at once.
        failToCli();
        return;
    }
    ++m_initialFailures;
    if (m_initialFailures < 2) {
        // One activation retry: connecting to a systemd socket unit may have
        // just started the daemon, so give it a short moment.
        m_state = State::Reconnecting;
        m_reconnectTimer.start(100);
        m_backoffMs = 250;
        return;
    }
    failToCli();
}

void SocketTransport::failToCli() {
    clearHandshakeTimer();
    m_reconnectTimer.stop();
    m_state = State::Failed;
    m_suppressDisconnect = true;
    m_socket->abort();
    m_suppressDisconnect = false;
    emit fellBackToCli();
}

void SocketTransport::handshakeFailed(const QString &) {
    clearHandshakeTimer();
    m_suppressDisconnect = true;
    m_socket->abort();
    m_suppressDisconnect = false;
    if (m_everReady) {
        handleDrop();
    } else {
        handleSocketFailure();
    }
}

void SocketTransport::handleDrop() {
    if (m_state == State::Reconnecting || m_state == State::Failed) return;
    m_state = State::Reconnecting;
    clearHandshakeTimer();
    failPending(QStringLiteral("connection_lost"), tr("Salah daemon connection was lost."));
    emit readyChanged(false);
    scheduleReconnect();
}

void SocketTransport::clearHandshakeTimer() {
    m_handshakeTimer.stop();
    m_handshakeId = -1;
}

void SocketTransport::failPending(const QString &kind, const QString &message) {
    const auto pending = m_pending;
    m_pending.clear();
    for (const Pending &entry : pending) {
        if (entry.timer) {
            entry.timer->stop();
            entry.timer->deleteLater();
        }
        Reply reply;
        reply.kind = kind;
        reply.message = message;
        if (entry.handler) entry.handler(reply);
    }
}

void SocketTransport::writeRequest(qint64 id, const QString &method, const QJsonObject &params) {
    QJsonObject request{{"jsonrpc", QStringLiteral("2.0")}, {"id", id}, {"method", method}};
    if (!params.isEmpty()) request.insert(QStringLiteral("params"), params);
    QByteArray line = QJsonDocument(request).toJson(QJsonDocument::Compact);
    line.push_back('\n');
    if (m_socket->write(line) != line.size() || !m_socket->flush()) {
        handleSocketFailure();
    }
}

void SocketTransport::send(const QString &method, const QJsonObject &params, Handler handler) {
    if (m_state != State::Ready) {
        Reply reply;
        reply.kind = QStringLiteral("not_ready");
        reply.message = tr("Salah daemon is not reachable.");
        if (handler) handler(reply);
        return;
    }
    const qint64 id = m_nextId++;
    auto *timer = new QTimer(this);
    timer->setSingleShot(true);
    m_pending.insert(id, Pending{std::move(handler), timer});
    connect(timer, &QTimer::timeout, this, [this, id] {
        auto it = m_pending.find(id);
        if (it == m_pending.end()) return;
        Pending entry = it.value();
        m_pending.erase(it);
        if (entry.timer) {
            entry.timer->stop();
            entry.timer->deleteLater();
        }
        Reply reply;
        reply.kind = QStringLiteral("timeout");
        reply.message = tr("Salah request timed out.");
        if (entry.handler) entry.handler(reply);
    });
    timer->start(m_timeoutMs);
    writeRequest(id, method, params);
}

void SocketTransport::finishReady() {
    clearHandshakeTimer();
    const bool wasReady = m_everReady;
    m_everReady = true;
    m_initialFailures = 0;
    m_backoffMs = 250;
    m_state = State::Ready;
    emit readyChanged(true);
    if (wasReady) emit reconnected();
}

void SocketTransport::handleHandshake(const QJsonObject &object, qint64 id) {
    if (id != m_handshakeId) return;
    if (object.contains(QStringLiteral("error"))) {
        const QJsonObject error = object.value(QStringLiteral("error")).toObject();
        handshakeFailed(error.value(QStringLiteral("message")).toString());
        return;
    }
    const QJsonObject result = object.value(QStringLiteral("result")).toObject();
    if (m_state == State::Hello) {
        if (result.value(QStringLiteral("protocol")).toInt() != 1) {
            handshakeFailed(tr("Salah daemon does not speak protocol 1."));
            return;
        }
        if (result.contains(QStringLiteral("revision"))) {
            m_lastRevision = qMax(m_lastRevision, result.value(QStringLiteral("revision")).toVariant().toLongLong());
        }
        m_state = State::Subscribe;
        m_handshakeId = m_nextId++;
        QJsonObject params{{"topics", QJsonArray{QStringLiteral("changes")}}};
        if (m_lastRevision > 0) params.insert(QStringLiteral("after_revision"), m_lastRevision);
        writeRequest(m_handshakeId, QStringLiteral("subscribe"), params);
        m_handshakeTimer.start(m_timeoutMs);
        return;
    }
    if (m_state == State::Subscribe) {
        if (result.contains(QStringLiteral("revision"))) {
            m_lastRevision = qMax(m_lastRevision, result.value(QStringLiteral("revision")).toVariant().toLongLong());
        }
        finishReady();
    }
}

void SocketTransport::onReadyRead() {
    m_buffer += m_socket->readAll();
    for (;;) {
        const int newline = m_buffer.indexOf('\n');
        if (newline < 0) {
            if (m_buffer.size() > kMaxLineBytes) {
                m_buffer.clear();
                handshakeFailed(tr("Salah daemon sent an oversized frame."));
            }
            return;
        }
        QByteArray line = m_buffer.left(newline);
        m_buffer.remove(0, newline + 1);
        if (line.isEmpty()) continue;
        if (line.size() > kMaxLineBytes) {
            handshakeFailed(tr("Salah daemon sent an oversized frame."));
            return;
        }
        QJsonParseError parseError;
        const QJsonDocument document = QJsonDocument::fromJson(line, &parseError);
        if (parseError.error != QJsonParseError::NoError || !document.isObject()) continue;
        const QJsonObject object = document.object();
        if (!object.contains(QStringLiteral("id"))) {
            if (object.value(QStringLiteral("method")).toString() == QStringLiteral("changed")) {
                const qint64 revision = object.value(QStringLiteral("params")).toObject()
                                            .value(QStringLiteral("revision"))
                                            .toVariant().toLongLong();
                m_lastRevision = qMax(m_lastRevision, revision);
                emit changed(revision);
            }
            continue;
        }
        const qint64 id = object.value(QStringLiteral("id")).toVariant().toLongLong();
        if (id == m_handshakeId) {
            handleHandshake(object, id);
            continue;
        }
        auto it = m_pending.find(id);
        if (it == m_pending.end()) continue;
        Pending entry = it.value();
        m_pending.erase(it);
        if (entry.timer) {
            entry.timer->stop();
            entry.timer->deleteLater();
        }
        Reply reply;
        if (object.contains(QStringLiteral("error"))) {
            const QJsonObject error = object.value(QStringLiteral("error")).toObject();
            const QJsonObject data = error.value(QStringLiteral("data")).toObject();
            reply.kind = data.value(QStringLiteral("kind")).toString();
            if (reply.kind.isEmpty()) {
                reply.kind = error.value(QStringLiteral("code")).toInt() == -32004
                                 ? QStringLiteral("conflict")
                                 : QStringLiteral("rpc");
            }
            reply.message = error.value(QStringLiteral("message")).toString();
            reply.conflict = reply.kind == QStringLiteral("conflict");
        } else {
            reply.ok = true;
            reply.value = object.value(QStringLiteral("result"));
        }
        if (entry.handler) entry.handler(reply);
    }
}
