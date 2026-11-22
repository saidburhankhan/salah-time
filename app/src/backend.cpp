#include "backend.h"

#include <QCalendar>
#include <QJsonArray>
#include <QJsonObject>
#include <QLocale>

Backend::Backend(QObject *parent, const QString &program, int timeoutMs)
    : QObject(parent), m_client(this, program, timeoutMs) {
    connect(&m_client, &CoreClient::busyChanged, this, &Backend::changed);
    connect(&m_client, &CoreClient::requestFinished, this, [this](const QStringList &args, bool success) {
        emit requestFinished(args, success);
        if (!m_client.busy() && m_refreshPending) {
            m_refreshPending = false;
            reload();
        }
    });
}

Backend::~Backend() = default;

CoreClient::Callback Backend::response(std::function<void(const QJsonObject &)> success, bool reloadAfter) {
    return [this, success = std::move(success), reloadAfter](const CoreClient::Result &result) {
        if (!result.ok) fail(result.error.message);
        else {
            if (success) success(result.value);
            if (reloadAfter) m_refreshPending = true;
        }
        emit changed();
    };
}

void Backend::fail(const QString &message) { m_error = message; }
void Backend::dismissError() { m_error.clear(); emit changed(); }

QVariantMap Backend::normalizedObject(const QJsonObject &object) { return object.toVariantMap(); }

bool Backend::setJsonModel(QVariantMap &target, QByteArray &previous, const QJsonObject &object) {
    const QByteArray json = QJsonDocument(object).toJson(QJsonDocument::Compact);
    if (json == previous) return false;
    previous = json;
    target = normalizedObject(object);
    return true;
}

void Backend::reload() {
    m_client.day(m_selected, response([this](const QJsonObject &object) {
        const bool modelChanged = setJsonModel(m_dayPlan, m_dayJson, object);
        const QDate cliDay = QDate::fromString(object.value("day").toString(), Qt::ISODate);
        if (cliDay.isValid()) {
            m_selected = cliDay;
            if (object.value("is_today").toBool()) m_today = cliDay;
        }
        if (modelChanged) { ++m_dayRevision; emit changed(); }
    }));
    m_client.statsDays(14, response([this](const QJsonObject &object) {
        if (setJsonModel(m_routinesStats, m_routinesJson, object)) emit changed();
    }));
    m_client.configShow(response([this](const QJsonObject &object) {
        if (setJsonModel(m_config, m_configJson, object)) emit changed();
    }));
}

void Backend::loadStats(int days) {
    if (days != 7 && days != 30) days = 30;
    m_client.statsDays(days, response([this](const QJsonObject &object) {
        if (setJsonModel(m_stats, m_statsJson, object)) emit changed();
    }));
}

void Backend::loadStatsRange(const QString &from, const QString &to) {
    const QDate first = QDate::fromString(from, Qt::ISODate);
    const QDate last = QDate::fromString(to, Qt::ISODate);
    if (!first.isValid() || !last.isValid() || first > last || first.daysTo(last) >= 366) {
        fail(tr("Invalid statistics date range.")); emit changed(); return;
    }
    m_client.stats(first, last, response([this](const QJsonObject &object) {
        if (setJsonModel(m_stats, m_statsJson, object)) emit changed();
    }));
}

void Backend::moveDay(int amount) { if (!m_selected.isValid()) m_selected = m_today; m_selected = m_selected.addDays(amount); reload(); }
void Backend::selectDay(const QString &day) { const QDate parsed = QDate::fromString(day, Qt::ISODate); if (parsed.isValid() && parsed != m_selected) { m_selected = parsed; reload(); } }
void Backend::jumpToday() { m_selected = m_today; reload(); }
void Backend::parseQuick(const QString &text) {
    if (text.trimmed().isEmpty()) { m_parseDraft.clear(); emit changed(); return; }
    m_client.parse(text, response([this](const QJsonObject &object) {
        m_parseDraft = normalizedObject(object);
        emit changed();
    }));
}

void Backend::addQuick(const QString &text, const QString &block) {
    insertPendingItem({{"id",QString("pending-%1").arg(QDateTime::currentMSecsSinceEpoch())},{"title",text},{"block",block.isEmpty()?QString("fajr"):block},{"repeat","none"},{"measure","check"},{"state","open"},{"pending",true}});
    CoreClient::ItemFields fields;
    fields.title = text; fields.block = block; fields.quick = true;
    if (m_selected.isValid() && m_selected != m_today) fields.date = m_selected;
    m_client.itemAdd(fields, response({}, true));
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
    CoreClient::ItemFields fields;
    fields.title = item.value("title").toString();
    fields.block = item.value("block", "fajr").toString();
    fields.repeat = item.value("repeat", "none").toString();
    fields.weekdays = item.value("weekdays").toStringList();
    fields.measure = item.value("measure", "check").toString();
    const int target = item.value("target").toInt();
    fields.hasTarget = fields.measure != "check" && target > 0;
    fields.target = target;
    if (id.isEmpty()) {
        if (fields.repeat == "none" && m_selected != m_today) fields.date = m_selected;
        m_client.itemAdd(fields, response({}, true));
    } else {
        m_client.itemEdit(id, fields, response({}, true));
    }
}

void Backend::removeItem(const QString &id) { QVariantList blocks=m_dayPlan.value("blocks").toList();bool removed=false;for(int b=0;b<blocks.size();++b){QVariantMap bm=blocks[b].toMap();QVariantList items=bm.value("items").toList();for(int i=0;i<items.size();++i)if(items[i].toMap().value("id").toString()==id){items.removeAt(i);bm["items"]=items;blocks[b]=bm;removed=true;break;}if(removed)break;}if(removed){m_dayPlan["blocks"]=blocks;m_dayJson.clear();++m_dayRevision;emit changed();}m_client.itemRemove(id,response({},true)); }
void Backend::moveItem(const QString &id, const QString &block, int index) {
    const int requestedIndex = index;
    QVariantList blocks = m_dayPlan.value("blocks").toList(); QVariantMap moved; int sourceBlock = -1;
    for (int b=0;b<blocks.size();++b) { QVariantMap bm=blocks[b].toMap(); QVariantList items=bm.value("items").toList(); for(int i=0;i<items.size();++i) if(items[i].toMap().value("id").toString()==id){moved=items.takeAt(i).toMap();bm["items"]=items;blocks[b]=bm;sourceBlock=b;break;} if(sourceBlock>=0)break; }
    for(int b=0;b<blocks.size()&&!moved.isEmpty();++b){QVariantMap bm=blocks[b].toMap();if(bm.value("start").toString()!=block)continue;QVariantList items=bm.value("items").toList();index=qBound(0,index,items.size());moved["block"]=block;items.insert(index,moved);for(int i=0;i<items.size();++i){QVariantMap x=items[i].toMap();x["position"]=i;items[i]=x;}bm["items"]=items;blocks[b]=bm;}
    if(!moved.isEmpty()){m_dayPlan["blocks"]=blocks;m_dayJson.clear();++m_dayRevision;emit changed();}
    m_client.itemMove(id, block, requestedIndex, response({}, true));
}
void Backend::toggleDone(const QString &id, const QString &state) { mutateItem(id,[state](QVariantMap &x){x["state"]=state=="done"?"open":"done";}); if(state=="done")m_client.itemUndo(id,m_selected,response({},true));else m_client.itemDone(id,m_selected,response({},true)); }
void Backend::adjustMinutes(const QString &id, int amount) {
    mutateItem(id, [amount](QVariantMap &x) { x["minutes"] = qMax(0, x.value("minutes").toInt() + amount); });
    m_client.itemLog(id, amount, true, m_selected != m_today ? m_selected : QDate(), response({}, true));
}
void Backend::adjustCount(const QString &id, int amount) {
    mutateItem(id, [amount](QVariantMap &x) { x["count"] = qMax(0, x.value("count").toInt() + amount); });
    m_client.itemLog(id, amount, false, m_selected != m_today ? m_selected : QDate(), response({}, true));
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
    if (running) m_client.timerStop(id, response({}, true)); else m_client.timerStart(id, response({}, true));
}
void Backend::togglePrayer(const QString &prayer, bool marked) { QVariantList prayers=m_dayPlan.value("prayers").toList();for(int i=0;i<prayers.size();++i){QVariantMap p=prayers[i].toMap();if(p.value("id").toString()==prayer){p["marked"]=!marked;prayers[i]=p;m_dayPlan["prayers"]=prayers;m_dayJson.clear();++m_dayRevision;emit changed();break;}} if(marked)m_client.prayerUnmark(prayer,m_selected,response({},true));else m_client.prayerMark(prayer,m_selected,response({},true)); }
void Backend::setSleepTime(const QString &time) { m_config["sleep_time"]=time;QVariantMap sleep=m_dayPlan.value("sleep").toMap();sleep["time"]=time;m_dayPlan["sleep"]=sleep;m_configJson.clear();m_dayJson.clear();++m_dayRevision;emit changed();m_client.configSetSleepTime(time,response({},true)); }

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
