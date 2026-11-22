#include "backend.h"

#include <QCalendar>
#include <QJsonArray>
#include <QJsonObject>
#include <QLocale>

Backend::Backend(QObject *parent, const QString &program, int timeoutMs)
    : QObject(parent), m_program(program.isEmpty() ? qEnvironmentVariable("SALAH_BIN", "salah") : program),
      m_timeoutMs(timeoutMs) {
    m_timeout.setSingleShot(true);
    connect(&m_timeout, &QTimer::timeout, this, [this] { finish(true); });
    connect(&m_process, &QProcess::finished, this, [this](int, QProcess::ExitStatus) {
        if (m_running) finish(); else startNext();
    });
    connect(&m_process, &QProcess::errorOccurred, this, [this](QProcess::ProcessError error) {
        if (error == QProcess::FailedToStart) finish();
    });
}

Backend::~Backend() {
    m_timeout.stop();
    m_queue.clear();
    m_running = false;
    m_process.disconnect(this);
    if (m_process.state() != QProcess::NotRunning) {
        m_process.kill();
        m_process.waitForFinished(500);
    }
}

void Backend::enqueue(const QStringList &args, std::function<void(const QJsonDocument &)> success,
                      bool reloadAfter, bool plainJson) {
    m_queue.enqueue({args, std::move(success), reloadAfter, plainJson});
    emit changed();
    startNext();
}

void Backend::startNext() {
    if (m_running || m_queue.isEmpty()) return;
    m_current = m_queue.dequeue();
    m_running = true;
    m_process.setProgram(m_program);
    m_process.setArguments(m_current.args);
    m_process.start();
    m_timeout.start(m_timeoutMs);
    emit changed();
}

void Backend::finish(bool timedOut) {
    if (!m_running) return;
    m_timeout.stop();
    if (timedOut) {
        const auto args = m_current.args;
        m_running = false;
        m_process.kill();
        fail(tr("Salah command timed out."));
        emit requestFinished(args, false);
        emit changed();
        return;
    }
    const bool success = !timedOut && m_process.exitStatus() == QProcess::NormalExit && m_process.exitCode() == 0;
    bool handled = success;
    if (!success) {
        QString detail = timedOut ? tr("Salah command timed out.")
                                  : QString::fromUtf8(m_process.readAllStandardError()).trimmed();
        if (detail.isEmpty()) detail = tr("Could not run Salah: %1").arg(m_process.errorString());
        fail(detail);
    } else {
        const QByteArray bytes = m_process.readAllStandardOutput();
        QJsonParseError parse;
        const QJsonDocument doc = QJsonDocument::fromJson(bytes, &parse);
        if (parse.error != QJsonParseError::NoError && !m_current.plainJson) {
            fail(tr("Salah returned invalid JSON: %1").arg(parse.errorString())); handled = false;
        } else if (m_current.success) m_current.success(doc);
    }
    const auto args = m_current.args;
    const bool reloadAfter = handled && m_current.reloadAfter;
    m_running = false;
    emit requestFinished(args, handled);
    emit changed();
    if (reloadAfter) reload();
    startNext();
}

void Backend::fail(const QString &message) { m_error = message; }
void Backend::dismissError() { m_error.clear(); emit changed(); }

QVariantMap Backend::normalizedObject(const QJsonObject &object) { return object.toVariantMap(); }

void Backend::reload() {
    QStringList args{"day"};
    if (m_selected.isValid()) args << "--date" << selectedDay();
    args << "--json";
    enqueue(args, [this](const QJsonDocument &doc) {
        if (!doc.isObject()) { fail(tr("Salah returned invalid day data.")); return; }
        m_dayPlan = normalizedObject(doc.object());
        const QDate cliDay = QDate::fromString(doc.object().value("day").toString(), Qt::ISODate);
        if (cliDay.isValid()) {
            m_selected = cliDay;
            if (doc.object().value("is_today").toBool()) m_today = cliDay;
        }
        emit changed();
    });
    enqueue({"stats", "--days", "14", "--json"}, [this](const QJsonDocument &doc) {
        if (doc.isObject()) m_routinesStats = normalizedObject(doc.object());
        else fail(tr("Salah returned invalid routine data."));
        emit changed();
    });
    enqueue({"config", "show"}, [this](const QJsonDocument &doc) {
        if (doc.isObject()) m_config = normalizedObject(doc.object());
        else fail(tr("Salah returned invalid configuration data."));
        emit changed();
    });
}

void Backend::loadStats(int days) {
    if (days != 7 && days != 30) days = 30;
    enqueue({"stats", "--days", QString::number(days), "--json"}, [this](const QJsonDocument &doc) {
        if (doc.isObject()) m_stats = normalizedObject(doc.object());
        else fail(tr("Salah returned invalid statistics data."));
        emit changed();
    });
}

void Backend::moveDay(int amount) { if (!m_selected.isValid()) m_selected = m_today; m_selected = m_selected.addDays(amount); reload(); }
void Backend::selectDay(const QString &day) { const QDate parsed = QDate::fromString(day, Qt::ISODate); if (parsed.isValid() && parsed != m_selected) { m_selected = parsed; reload(); } }
void Backend::jumpToday() { m_selected = m_today; reload(); }
QStringList Backend::dateArguments() const { return {"--date", selectedDay()}; }

void Backend::parseQuick(const QString &text) {
    if (text.trimmed().isEmpty()) { m_parseDraft.clear(); emit changed(); return; }
    enqueue({"item", "parse", "--json", "--", text}, [this](const QJsonDocument &doc) {
        if (doc.isObject()) m_parseDraft = normalizedObject(doc.object());
        emit changed();
    });
}

void Backend::addQuick(const QString &text, const QString &block) {
    QStringList args{"item", "add", "--json", "--quick"};
    if (!block.isEmpty()) args << "--block" << block;
    if (m_selected.isValid() && m_selected != m_today) args << "--date" << selectedDay();
    args << "--" << text; enqueue(args, {}, true);
}

void Backend::saveItem(const QVariantMap &item) {
    const QString id = item.value("id").toString();
    QStringList args{"item", id.isEmpty() ? "add" : "edit", "--json"};
    if (!id.isEmpty()) args << id << "--title" << item.value("title").toString();
    args << "--block" << item.value("block", "fajr").toString() << "--repeat" << item.value("repeat", "none").toString();
    const QStringList days = item.value("weekdays").toStringList();
    if (!days.isEmpty()) args << "--days" << days.join(',');
    args << "--measure" << item.value("measure", "check").toString();
    if (item.value("measure").toString() != "check") args << "--target" << QString::number(item.value("target", 1).toInt());
    else if (!id.isEmpty()) args << "--no-target";
    if (id.isEmpty()) {
        if (item.value("repeat").toString() == "none" && m_selected != m_today) args << "--date" << selectedDay();
        args << "--" << item.value("title").toString();
    }
    enqueue(args, {}, true);
}

void Backend::removeItem(const QString &id) { enqueue({"item", "remove", "--json", id}, {}, true); }
void Backend::moveItem(const QString &id, const QString &block, int index) { enqueue({"item", "move", "--json", id, "--block", block, "--index", QString::number(index)}, {}, true); }
void Backend::toggleDone(const QString &id, const QString &state) { QStringList args{"item", state == "done" ? "undo" : "done", "--json", id}; args << dateArguments(); enqueue(args, {}, true); }
void Backend::logOne(const QString &id) { QStringList args{"item", "log", "--json", id, "1"}; args << dateArguments(); enqueue(args, {}, true); }
void Backend::toggleTimer(const QString &id, bool running) { enqueue({"item", running ? "stop" : "start", "--json", id}, {}, true); }
void Backend::togglePrayer(const QString &prayer, bool marked) { QStringList args{"prayer", marked ? "unmark" : "mark", "--json", prayer}; args << dateArguments(); enqueue(args, {}, true); }
void Backend::setSleepTime(const QString &time) { enqueue({"config", "set", "--sleep-time", time}, {}, true, true); }

QString Backend::hijriHeader(const QString &isoDate) const {
    const QDate date = QDate::fromString(isoDate, Qt::ISODate); if (!date.isValid()) return {};
    const QCalendar calendar(QCalendar::System::IslamicCivil); const auto parts = calendar.partsFromDate(date);
    static const QStringList months{tr("Muharram"), tr("Safar"), tr("Rabi al-Awwal"), tr("Rabi al-Thani"), tr("Jumada al-Awwal"), tr("Jumada al-Thani"), tr("Rajab"), tr("Sha'ban"), tr("Ramadan"), tr("Shawwal"), tr("Dhu al-Qadah"), tr("Dhu al-Hijjah")};
    const QString month = parts.month >= 1 && parts.month <= months.size() ? months.at(parts.month - 1) : QString();
    return tr("%1 %2 %3").arg(QLocale::c().dayName(date.dayOfWeek(), QLocale::LongFormat), QString::number(parts.day), month);
}

QString Backend::gregorianHeader(const QString &isoDate) const { const QDate date = QDate::fromString(isoDate, Qt::ISODate); return date.isValid() ? QLocale::c().toString(date, "MMMM d, yyyy") : QString(); }
