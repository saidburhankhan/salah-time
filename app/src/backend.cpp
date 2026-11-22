#include "backend.h"

#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QProcess>

Backend::Backend(QObject *parent, const QString &program) : QObject(parent) {
    m_program = program.isEmpty() ? qEnvironmentVariable("SALAH_BIN", "salah") : program;
    m_today = QDate::currentDate();
}

QByteArray Backend::run(const QStringList &arguments, bool *ok) {
    QProcess process;
    process.start(m_program, arguments);
    if (!process.waitForStarted(3000) || !process.waitForFinished(10000)) {
        process.kill();
        m_error = tr("Could not run Salah: %1").arg(process.errorString());
        if (ok) *ok = false;
        return {};
    }
    const bool success = process.exitStatus() == QProcess::NormalExit && process.exitCode() == 0;
    if (!success) {
        const QString detail = QString::fromUtf8(process.readAllStandardError()).trimmed();
        m_error = detail.isEmpty() ? tr("Salah command failed (exit %1)").arg(process.exitCode()) : detail;
    }
    if (ok) *ok = success;
    return process.readAllStandardOutput();
}

void Backend::reload() {
    m_busy = true; m_error.clear(); emit changed();
    bool ok = false;
    const auto todayBytes = run({"item", "list", "--json"}, &ok);
    if (!ok) { m_busy = false; emit changed(); return; }
    QJsonParseError parse;
    const auto todayDoc = QJsonDocument::fromJson(todayBytes, &parse);
    if (parse.error != QJsonParseError::NoError || !todayDoc.isObject()) {
        m_error = tr("Salah returned invalid day data: %1").arg(parse.errorString());
        m_busy = false; emit changed(); return;
    }
    const auto today = todayDoc.object();
    const QDate cliToday = QDate::fromString(today.value("day").toString(), Qt::ISODate);
    if (cliToday.isValid()) m_today = cliToday;
    if (!m_selected.isValid()) m_selected = m_today;
    m_planningBlock = today.value("planning_block").toObject().value("start").toString("fajr");
    const QDate from = m_selected.addDays(-13);
    const auto historyBytes = run({"history", "--from", from.toString(Qt::ISODate), "--to",
                                   m_selected.toString(Qt::ISODate), "--json"}, &ok);
    if (!ok) { m_busy = false; emit changed(); return; }
    const auto historyDoc = QJsonDocument::fromJson(historyBytes, &parse);
    if (parse.error != QJsonParseError::NoError || !historyDoc.isArray()) {
        m_error = tr("Salah returned invalid history data: %1").arg(parse.errorString());
        m_busy = false; emit changed(); return;
    }
    rebuild(historyDoc.array());
    m_busy = false; emit changed();
}

static QVariantMap itemMap(const QJsonObject &o) {
    QVariantMap m = o.toVariantMap();
    m["target"] = o.value("target").isNull() ? 0 : o.value("target").toInt();
    return m;
}

void Backend::rebuild(const QJsonArray &reports) {
    m_days.clear(); m_selectedItems.clear(); m_routines.clear();
    QMap<QString, QVariantMap> daily;
    for (const auto &rv : reports) {
        const auto report = rv.toObject();
        const QString day = report.value("day").toString();
        int done = 0;
        QVariantList items;
        for (const auto &iv : report.value("items").toArray()) {
            const auto io = iv.toObject();
            auto im = itemMap(io); items << im;
            if (io.value("state").toString() == "done") ++done;
            if (io.value("repeat").toString() == "daily") {
                auto r = daily.value(io.value("id").toString());
                if (r.isEmpty()) {
                    r = im; r["values"] = QVariantList(); r["states"] = QVariantList(); r["total"] = 0;
                }
                auto values = r["values"].toList(); auto states = r["states"].toList();
                const QString measure = io.value("measure").toString();
                const int value = measure == "minutes" ? io.value("minutes").toInt() :
                                  measure == "count" ? io.value("count").toInt() :
                                  (io.value("state").toString() == "done" ? 1 : 0);
                values << value; states << io.value("state").toString();
                r["values"] = values; r["states"] = states; r["total"] = r["total"].toInt() + value;
                daily[io.value("id").toString()] = r;
            }
        }
        m_days << QVariantMap{{"day", day}, {"done", done}, {"planned", items.size()}};
        if (day == selectedDay()) m_selectedItems = items;
    }
    for (auto it = daily.cbegin(); it != daily.cend(); ++it) m_routines << it.value();
}

void Backend::moveDay(int amount) { m_selected = m_selected.addDays(amount); reload(); }
void Backend::selectDay(const QString &day) { const auto d = QDate::fromString(day, Qt::ISODate); if (d.isValid()) { m_selected = d; reload(); } }
void Backend::jumpToday() { m_selected = m_today; reload(); }

void Backend::addItem(const QString &title, const QString &block, bool daily, const QString &measure, int target) {
    QStringList args{"item", "add", "--json", "--block", block};
    if (daily) args << "--daily";
    args << "--measure" << measure;
    if (measure != "check" && target > 0) args << "--target" << QString::number(target);
    args << "--" << title;
    bool ok = false; run(args, &ok); if (ok) reload(); else emit changed();
}

void Backend::toggleDone(const QString &id, const QString &state) {
    if (m_selected != m_today) { m_error = tr("Only today's items can be changed here."); emit changed(); return; }
    bool ok = false; run({"item", state == "done" ? "undo" : "done", "--json", id}, &ok);
    if (ok) reload(); else emit changed();
}
