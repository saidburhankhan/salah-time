#pragma once

#include <QHash>
#include <QJsonObject>
#include <QJsonValue>
#include <QLocalSocket>
#include <QObject>
#include <QTimer>
#include <functional>

// Protocol v1 (NDJSON JSON-RPC 2.0) client transport for the daemon socket.
//
// The transport owns the connection lifecycle: hello -> subscribe -> ready.
// If the socket cannot be reached after the initial connect attempt and one
// activation retry it emits fellBackToCli(); after a connection that was once
// ready drops it reconnects with exponential backoff.
class SocketTransport : public QObject {
    Q_OBJECT
public:
    struct Reply {
        bool ok = false;
        bool conflict = false;
        QString kind;
        QString message;
        QJsonValue value;
    };
    using Handler = std::function<void(const Reply &)>;

    explicit SocketTransport(QObject *parent = nullptr);
    ~SocketTransport() override;

    void setSocketPath(const QString &path) { m_path = path; }
    QString socketPath() const { return m_path; }
    void setTimeoutMs(int ms) { m_timeoutMs = ms; }
    int timeoutMs() const { return m_timeoutMs; }
    bool ready() const { return m_state == State::Ready; }
    qint64 lastRevision() const { return m_lastRevision; }

    void start();
    void stop();
    void send(const QString &method, const QJsonObject &params, Handler handler);

    static QString defaultSocketPath();

signals:
    void readyChanged(bool ready);
    void fellBackToCli();
    void reconnected();
    void changed(qint64 revision);

private:
    enum class State { Idle, Connecting, Hello, Subscribe, Ready, Reconnecting, Failed };

    struct Pending {
        Handler handler;
        QTimer *timer = nullptr;
    };

    void beginConnect();
    void scheduleReconnect();
    void handleSocketFailure();
    void handshakeFailed(const QString &message);
    void handleDrop();
    void failPending(const QString &kind, const QString &message);
    void onReadyRead();
    void writeRequest(qint64 id, const QString &method, const QJsonObject &params);
    void handleHandshake(const QJsonObject &object, qint64 id);
    void finishReady();
    void clearHandshakeTimer();

    QLocalSocket *m_socket = nullptr;
    QTimer m_reconnectTimer;
    QTimer m_handshakeTimer;
    QString m_path;
    int m_timeoutMs = 10000;
    State m_state = State::Idle;
    qint64 m_nextId = 1;
    qint64 m_handshakeId = -1;
    int m_initialFailures = 0;
    int m_backoffMs = 250;
    bool m_everReady = false;
    bool m_suppressDisconnect = false;
    qint64 m_lastRevision = 0;
    QByteArray m_buffer;
    QHash<qint64, Pending> m_pending;
};
