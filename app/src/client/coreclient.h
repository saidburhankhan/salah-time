#pragma once

#include <QDate>
#include <QObject>
#include <QProcess>
#include <QQueue>
#include <QTimer>
#include <functional>
#include "dto.h"

class CoreClient : public QObject {
    Q_OBJECT
public:
    enum class ErrorKind { Process, Exit, Decode, Timeout };
    struct Error { ErrorKind kind; QString message; };
    struct Result {
        Dto::Value value;
        Error error{ErrorKind::Process, {}};
        bool ok = false;
    };
    struct ItemFields {
        QString title;
        QString block = "fajr";
        QString repeat = "none";
        QStringList weekdays;
        QString measure = "check";
        int target = 0;
        bool hasTarget = false;
        QDate date;
        bool quick = false;
    };
    using Callback = std::function<void(const Result &)>;

    explicit CoreClient(QObject *parent = nullptr, const QString &program = {}, int timeoutMs = 10000);
    ~CoreClient() override;
    bool busy() const { return m_running || !m_queue.isEmpty(); }

    void day(const QDate &date, Callback callback);
    void stats(const QDate &from, const QDate &to, Callback callback);
    void statsDays(int days, Callback callback);
    void itemAdd(const ItemFields &fields, Callback callback = {});
    void itemEdit(const QString &id, const ItemFields &fields, Callback callback = {});
    void itemRemove(const QString &id, Callback callback = {});
    void itemMove(const QString &id, const QString &block, int index, Callback callback = {});
    void itemDone(const QString &id, const QDate &date, Callback callback = {});
    void itemUndo(const QString &id, const QDate &date, Callback callback = {});
    void itemLog(const QString &id, int value, bool minutes, const QDate &date = {}, Callback callback = {});
    void timerStart(const QString &id, Callback callback = {});
    void timerStop(const QString &id, Callback callback = {});
    void prayerMark(const QString &prayer, const QDate &date, Callback callback = {});
    void prayerUnmark(const QString &prayer, const QDate &date, Callback callback = {});
    void parse(const QString &text, Callback callback);
    void configShow(Callback callback);
    void configSetSleepTime(const QString &time, Callback callback = {});

signals:
    void busyChanged();
    void requestFinished(QStringList arguments, bool success);

private:
    struct Request {
        QStringList args;
        QList<Callback> callbacks;
        bool read = false;
        bool expectJson = true;
        std::function<Dto::AnyDecoded(const QJsonValue &)> decoder;
    };
    void enqueue(const QStringList &args, Callback callback, bool read, std::function<Dto::AnyDecoded(const QJsonValue &)> decoder);
    void enqueueNoJson(const QStringList &args, Callback callback);
    void startNext();
    void finish(bool timedOut = false);
    static QStringList dateArgs(const QDate &date);
    static QString stripErrorPrefix(QString message);

    QString m_program;
    int m_timeoutMs;
    QQueue<Request> m_queue;
    Request m_current;
    QProcess m_process;
    QTimer m_timeout;
    bool m_running = false;
};

