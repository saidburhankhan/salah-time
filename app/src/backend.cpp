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
    if (m_running) return;
    if (m_queue.isEmpty()) {
        if (m_refreshPending) { m_refreshPending = false; reload(); }
        return;
    }
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
    if (reloadAfter) m_refreshPending = true;
    startNext();
}

void Backend::fail(const QString &message) { m_error = message; }
void Backend::dismissError() { m_error.clear(); emit changed(); }

QVariantMap Backend::normalizedObject(const QJsonObject &object) { return object.toVariantMap(); }

bool Backend::setJsonModel(QVariantMap &target, QByteArray &previous, const QJsonDocument &doc) {
    if (!doc.isObject()) return false;
    const QByteArray json = doc.toJson(QJsonDocument::Compact);
    if (json == previous) return false;
    previous = json;
    target = normalizedObject(doc.object());
    return true;
}

void Backend::reload() {
    QStringList args{"day"};
    if (m_selected.isValid()) args << "--date" << selectedDay();
    args << "--json";
    enqueue(args, [this](const QJsonDocument &doc) {
        if (!doc.isObject()) { fail(tr("Salah returned invalid day data.")); return; }
        const bool modelChanged = setJsonModel(m_dayPlan, m_dayJson, doc);
        const QDate cliDay = QDate::fromString(doc.object().value("day").toString(), Qt::ISODate);
        if (cliDay.isValid()) {
            m_selected = cliDay;
            if (doc.object().value("is_today").toBool()) m_today = cliDay;
        }
        if (modelChanged) { ++m_dayRevision; emit changed(); }
    });
    enqueue({"stats", "--days", "14", "--json"}, [this](const QJsonDocument &doc) {
        if (doc.isObject()) { if (setJsonModel(m_routinesStats, m_routinesJson, doc)) emit changed(); }
        else fail(tr("Salah returned invalid routine data."));
    });
    enqueue({"config", "show"}, [this](const QJsonDocument &doc) {
        if (doc.isObject()) { if (setJsonModel(m_config, m_configJson, doc)) emit changed(); }
        else fail(tr("Salah returned invalid configuration data."));
    });
}

void Backend::loadStats(int days) {
    if (days != 7 && days != 30) days = 30;
    enqueue({"stats", "--days", QString::number(days), "--json"}, [this](const QJsonDocument &doc) {
        if (doc.isObject()) { if (setJsonModel(m_stats, m_statsJson, doc)) emit changed(); }
        else fail(tr("Salah returned invalid statistics data."));
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
    insertPendingItem({{"id",QString("pending-%1").arg(QDateTime::currentMSecsSinceEpoch())},{"title",text},{"block",block.isEmpty()?QString("fajr"):block},{"repeat","none"},{"measure","check"},{"state","open"},{"pending",true}});
    QStringList args{"item", "add", "--json", "--quick"};
    if (!block.isEmpty()) args << "--block" << block;
    if (m_selected.isValid() && m_selected != m_today) args << "--date" << selectedDay();
    args << "--" << text; enqueue(args, {}, true);
}

void Backend::mutateItem(const QString &id, const std::function<void(QVariantMap &)> &change) {
    QVariantList blocks = m_dayPlan.value("blocks").toList();
    bool found = false;
    for (int b = 0; b < blocks.size(); ++b) {
        QVariantMap block = blocks.at(b).toMap();
        QVariantList items = block.value("items").toList();
        for (int i = 0; i < items.size(); ++i) {
            QVariantMap item = items.at(i).toMap();
            if (item.value("id").toString() != id) continue;
            change(item); items[i] = item; block["items"] = items; blocks[b] = block; found = true; break;
        }
        if (found) break;
    }
    if (found) { m_dayPlan["blocks"] = blocks; m_dayJson.clear(); ++m_dayRevision; emit changed(); }
}

void Backend::insertPendingItem(const QVariantMap &source) {
    QVariantMap item=source; const QString target=item.value("block","fajr").toString(); QVariantList blocks=m_dayPlan.value("blocks").toList();
    for(int b=0;b<blocks.size();++b){QVariantMap bm=blocks[b].toMap();if(bm.value("start").toString()!=target)continue;QVariantList items=bm.value("items").toList();item["position"]=items.size();items.append(item);bm["items"]=items;blocks[b]=bm;m_dayPlan["blocks"]=blocks;m_dayJson.clear();++m_dayRevision;emit changed();break;}
}

void Backend::saveItem(const QVariantMap &item) {
    const QString id = item.value("id").toString();
    if(id.isEmpty()){QVariantMap pending=item;pending["id"]=QString("pending-%1").arg(QDateTime::currentMSecsSinceEpoch());pending["state"]="open";pending["pending"]=true;insertPendingItem(pending);}else mutateItem(id,[item](QVariantMap &x){for(auto i=item.cbegin();i!=item.cend();++i)x[i.key()]=i.value();});
    QStringList args{"item", id.isEmpty() ? "add" : "edit", "--json"};
    if (!id.isEmpty()) args << id << "--title=" + item.value("title").toString();
    args << "--block" << item.value("block", "fajr").toString() << "--repeat" << item.value("repeat", "none").toString();
    const QStringList days = item.value("weekdays").toStringList();
    if (!days.isEmpty()) args << "--days" << days.join(',');
    args << "--measure" << item.value("measure", "check").toString();
    const int target = item.value("target").toInt();
    if (item.value("measure").toString() != "check" && target > 0) args << "--target" << QString::number(target);
    else if (!id.isEmpty()) args << "--no-target";
    if (id.isEmpty()) {
        if (item.value("repeat").toString() == "none" && m_selected != m_today) args << "--date" << selectedDay();
        args << "--" << item.value("title").toString();
    }
    enqueue(args, {}, true);
}

void Backend::removeItem(const QString &id) { QVariantList blocks=m_dayPlan.value("blocks").toList();bool removed=false;for(int b=0;b<blocks.size();++b){QVariantMap bm=blocks[b].toMap();QVariantList items=bm.value("items").toList();for(int i=0;i<items.size();++i)if(items[i].toMap().value("id").toString()==id){items.removeAt(i);bm["items"]=items;blocks[b]=bm;removed=true;break;}if(removed)break;}if(removed){m_dayPlan["blocks"]=blocks;m_dayJson.clear();++m_dayRevision;emit changed();}enqueue({"item", "remove", "--json", id}, {}, true); }
void Backend::moveItem(const QString &id, const QString &block, int index) {
    const int requestedIndex = index;
    QVariantList blocks = m_dayPlan.value("blocks").toList(); QVariantMap moved; int sourceBlock = -1;
    for (int b=0;b<blocks.size();++b) { QVariantMap bm=blocks[b].toMap(); QVariantList items=bm.value("items").toList(); for(int i=0;i<items.size();++i) if(items[i].toMap().value("id").toString()==id){moved=items.takeAt(i).toMap();bm["items"]=items;blocks[b]=bm;sourceBlock=b;break;} if(sourceBlock>=0)break; }
    for(int b=0;b<blocks.size()&&!moved.isEmpty();++b){QVariantMap bm=blocks[b].toMap();if(bm.value("start").toString()!=block)continue;QVariantList items=bm.value("items").toList();index=qBound(0,index,items.size());moved["block"]=block;items.insert(index,moved);for(int i=0;i<items.size();++i){QVariantMap x=items[i].toMap();x["position"]=i;items[i]=x;}bm["items"]=items;blocks[b]=bm;}
    if(!moved.isEmpty()){m_dayPlan["blocks"]=blocks;m_dayJson.clear();++m_dayRevision;emit changed();}
    enqueue({"item", "move", "--json", id, "--block", block, "--index", QString::number(requestedIndex)}, {}, true);
}
void Backend::toggleDone(const QString &id, const QString &state) { mutateItem(id,[state](QVariantMap &x){x["state"]=state=="done"?"open":"done";}); QStringList args{"item", state == "done" ? "undo" : "done", "--json", id}; args << dateArguments(); enqueue(args, {}, true); }
void Backend::adjustMinutes(const QString &id, int amount) {
    mutateItem(id, [amount](QVariantMap &x) { x["minutes"] = qMax(0, x.value("minutes").toInt() + amount); });
    QStringList args{"item", "log", "--json", "--minutes", id, QString::number(amount)};
    if (m_selected.isValid() && m_selected != m_today) args << dateArguments();
    enqueue(args, {}, true);
}
void Backend::adjustCount(const QString &id, int amount) {
    mutateItem(id, [amount](QVariantMap &x) { x["count"] = qMax(0, x.value("count").toInt() + amount); });
    QStringList args{"item", "log", "--json", id, QString::number(amount)};
    if (m_selected.isValid() && m_selected != m_today) args << dateArguments();
    enqueue(args, {}, true);
}
void Backend::toggleTimer(const QString &id, bool running) {
    const QDateTime now = QDateTime::currentDateTimeUtc();
    const QString started = now.toString(Qt::ISODate);
    QVariantList blocks = m_dayPlan.value("blocks").toList();
    bool modelChanged = false;
    for (int b = 0; b < blocks.size(); ++b) {
        QVariantMap block = blocks.at(b).toMap(); QVariantList items = block.value("items").toList();
        for (int i = 0; i < items.size(); ++i) {
            QVariantMap item = items.at(i).toMap();
            if (running) {
                if (item.value("id").toString() == id) {
                    const QDateTime previous = QDateTime::fromString(item.value("timer_started_at").toString(), Qt::ISODate);
                    if (previous.isValid()) item["minutes"] = item.value("minutes").toInt() + qMax<qint64>(0, previous.secsTo(now) / 60);
                    item["timer_started_at"] = QVariant(); modelChanged = true;
                }
            } else if (item.value("id").toString() == id) {
                item["timer_started_at"] = started; modelChanged = true;
            } else if (!item.value("timer_started_at").toString().isEmpty()) {
                const QDateTime previous = QDateTime::fromString(item.value("timer_started_at").toString(), Qt::ISODate);
                if (previous.isValid()) item["minutes"] = item.value("minutes").toInt() + qMax<qint64>(0, previous.secsTo(now) / 60);
                item["timer_started_at"] = QVariant(); modelChanged = true;
            }
            items[i] = item;
        }
        block["items"] = items; blocks[b] = block;
    }
    if (modelChanged) { m_dayPlan["blocks"] = blocks; m_dayJson.clear(); ++m_dayRevision; emit changed(); }
    enqueue({"item", running ? "stop" : "start", "--json", id}, {}, true);
}
void Backend::togglePrayer(const QString &prayer, bool marked) { QVariantList prayers=m_dayPlan.value("prayers").toList();for(int i=0;i<prayers.size();++i){QVariantMap p=prayers[i].toMap();if(p.value("id").toString()==prayer){p["marked"]=!marked;prayers[i]=p;m_dayPlan["prayers"]=prayers;m_dayJson.clear();++m_dayRevision;emit changed();break;}} QStringList args{"prayer", marked ? "unmark" : "mark", "--json", prayer}; args << dateArguments(); enqueue(args, {}, true); }
void Backend::setSleepTime(const QString &time) { m_config["sleep_time"]=time;QVariantMap sleep=m_dayPlan.value("sleep").toMap();sleep["time"]=time;m_dayPlan["sleep"]=sleep;m_configJson.clear();m_dayJson.clear();++m_dayRevision;emit changed();enqueue({"config", "set", "--sleep-time", time}, {}, true, true); }

QVariantMap Backend::moveDestination(const QVariantList &rows, int gapIndex) const {
    gapIndex = qBound(0, gapIndex, rows.size()); QString block; int index = 0;
    for (int i=0;i<gapIndex;++i) { const QVariantMap row=rows.at(i).toMap(); const QString kind=row.value("kind").toString(); if(kind=="header"){block=row.value("block").toString();index=0;} else if(kind=="item"&&row.value("block").toString()==block) ++index; }
    return {{"block",block},{"index",index}};
}

QString Backend::hijriHeader(const QString &isoDate) const {
    const QDate date = QDate::fromString(isoDate, Qt::ISODate); if (!date.isValid()) return {};
    const QCalendar calendar(QCalendar::System::IslamicCivil); const auto parts = calendar.partsFromDate(date);
    static const QStringList months{tr("Muharram"), tr("Safar"), tr("Rabi al-Awwal"), tr("Rabi al-Thani"), tr("Jumada al-Awwal"), tr("Jumada al-Thani"), tr("Rajab"), tr("Sha'ban"), tr("Ramadan"), tr("Shawwal"), tr("Dhu al-Qadah"), tr("Dhu al-Hijjah")};
    const QString month = parts.month >= 1 && parts.month <= months.size() ? months.at(parts.month - 1) : QString();
    return tr("%1 %2 %3").arg(QLocale::c().dayName(date.dayOfWeek(), QLocale::LongFormat), QString::number(parts.day), month);
}

QString Backend::gregorianHeader(const QString &isoDate) const { const QDate date = QDate::fromString(isoDate, Qt::ISODate); return date.isValid() ? QLocale::c().toString(date, "MMMM d, yyyy") : QString(); }
