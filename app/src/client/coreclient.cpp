#include "coreclient.h"

#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonParseError>
#include <algorithm>

CoreClient::CoreClient(QObject *parent, const QString &program, int timeoutMs)
    : QObject(parent), m_program(program.isEmpty() ? qEnvironmentVariable("SALAH_BIN", "salah") : program),
      m_timeoutMs(timeoutMs) {
    m_timeout.setSingleShot(true);
    connect(&m_timeout, &QTimer::timeout, this, [this] { finish(true); });
    connect(&m_process, &QProcess::finished, this, [this](int, QProcess::ExitStatus) {
        if (m_running) finish();
    });
    connect(&m_process, &QProcess::errorOccurred, this, [this](QProcess::ProcessError error) {
        if (m_running && error == QProcess::FailedToStart) finish();
    });
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

void CoreClient::enqueue(const QStringList &args, Callback callback, bool read, bool expectJson) {
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
    Request request{args, {}, read, expectJson};
    if (callback) request.callbacks.append(std::move(callback));
    m_queue.enqueue(std::move(request));
    emit busyChanged();
    startNext();
}

void CoreClient::startNext() {
    if (m_running || m_queue.isEmpty()) return;
    m_current = m_queue.dequeue();
    m_running = true;
    m_process.setProgram(m_program);
    m_process.setArguments(m_current.args);
    m_process.start();
    m_timeout.start(m_timeoutMs);
    emit busyChanged();
}

QString CoreClient::stripErrorPrefix(QString message) {
    message = message.trimmed();
    if (message.startsWith("salah: ")) message.remove(0, 7);
    return message;
}

void CoreClient::finish(bool timedOut) {
    if (!m_running) return;
    m_timeout.stop();
    Result result;
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
        if (parseError.error == QJsonParseError::NoError && document.isArray()) {
            // Some commands (e.g. `item stop --json`) answer with a list.
            result.ok = true;
            result.value = QJsonObject{{"items", document.array()}};
        } else if (parseError.error != QJsonParseError::NoError || !document.isObject()) {
            const QString detail = parseError.error == QJsonParseError::NoError
                ? tr("JSON root is not an object or array") : parseError.errorString();
            result.error = {ErrorKind::Decode, tr("Salah returned invalid JSON: %1").arg(detail)};
        } else {
            result.ok = true;
            result.value = document.object();
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

QStringList CoreClient::dateArgs(const QDate &date) {
    return date.isValid() ? QStringList{"--date", date.toString(Qt::ISODate)} : QStringList{};
}

void CoreClient::day(const QDate &date, Callback callback) {
    QStringList args{"day"}; args << dateArgs(date) << "--json"; enqueue(args, std::move(callback), true);
}
void CoreClient::stats(const QDate &from, const QDate &to, Callback callback) {
    enqueue({"stats", "--from", from.toString(Qt::ISODate), "--to", to.toString(Qt::ISODate), "--json"}, std::move(callback), true);
}
void CoreClient::statsDays(int days, Callback callback) {
    enqueue({"stats", "--days", QString::number(days), "--json"}, std::move(callback), true);
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
    enqueue(args, std::move(callback));
}
void CoreClient::itemEdit(const QString &id, const ItemFields &f, Callback callback) {
    QStringList args{"item", "edit", "--json", id, "--title=" + f.title, "--block", f.block, "--repeat", f.repeat};
    if (!f.weekdays.isEmpty()) args << "--days" << f.weekdays.join(',');
    args << "--measure" << f.measure;
    if (f.hasTarget) args << "--target" << QString::number(f.target); else args << "--no-target";
    enqueue(args, std::move(callback));
}
void CoreClient::itemRemove(const QString &id, Callback callback) { enqueue({"item", "remove", "--json", id}, std::move(callback)); }
void CoreClient::itemMove(const QString &id, const QString &block, int index, Callback callback) { enqueue({"item", "move", "--json", id, "--block", block, "--index", QString::number(index)}, std::move(callback)); }
void CoreClient::itemDone(const QString &id, const QDate &date, Callback callback) { QStringList a{"item","done","--json",id}; a << dateArgs(date); enqueue(a,std::move(callback)); }
void CoreClient::itemUndo(const QString &id, const QDate &date, Callback callback) { QStringList a{"item","undo","--json",id}; a << dateArgs(date); enqueue(a,std::move(callback)); }
void CoreClient::itemLog(const QString &id, int value, bool minutes, const QDate &date, Callback callback) { QStringList a{"item","log","--json"}; if(minutes)a<<"--minutes"; a<<id<<QString::number(value); a<<dateArgs(date); enqueue(a,std::move(callback)); }
void CoreClient::timerStart(const QString &id, Callback callback) { enqueue({"item","start","--json",id},std::move(callback)); }
void CoreClient::timerStop(const QString &id, Callback callback) { enqueue({"item","stop","--json",id},std::move(callback)); }
void CoreClient::prayerMark(const QString &prayer, const QDate &date, Callback callback) { QStringList a{"prayer","mark","--json",prayer}; a<<dateArgs(date); enqueue(a,std::move(callback)); }
void CoreClient::prayerUnmark(const QString &prayer, const QDate &date, Callback callback) { QStringList a{"prayer","unmark","--json",prayer}; a<<dateArgs(date); enqueue(a,std::move(callback)); }
void CoreClient::parse(const QString &text, Callback callback) { enqueue({"item","parse","--json","--",text},std::move(callback),true); }
void CoreClient::configShow(Callback callback) { enqueue({"config","show"},std::move(callback),true); }
void CoreClient::configSetSleepTime(const QString &time, Callback callback) { enqueue({"config","set","--sleep-time",time},std::move(callback),false,false); }
