#include <QDir>
#include <QFile>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QQmlComponent>
#include <QQmlContext>
#include <QQmlEngine>
#include <QtTest>
#include "backend.h"
#include "theme.h"
#include "translator.h"

class SalahAppTests : public QObject {
    Q_OBJECT
    static void write(const QString &path, const QByteArray &data) { QFile f(path); QVERIFY(f.open(QIODevice::WriteOnly)); QCOMPARE(f.write(data), data.size()); }
    static QString fake(QTemporaryDir &dir) {
        const QString path=dir.filePath("salah"), log=dir.filePath("args");
        QByteArray script = "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '"+log.toUtf8()+"'\n";
        script += R"SH(
if [ "$5" = "slow" ]; then sleep 2; printf '{}\n'; exit 0; fi
if [ "$1 $2" = "item done" ]; then sleep 1; printf '{}\n'; exit 0; fi
if [ "$1" = "day" ]; then
 printf '%s\n' '{"day":"2026-11-22","is_today":true,"now":{"block":"dhuhr","sleeping":false,"next":{"id":"asr","name":"Asr","time":"15:18","remaining_seconds":7200}},"prayers":[{"id":"fajr","name":"Fajr","time":"05:58","marked":true},{"id":"dhuhr","name":"Dhuhr","time":"12:11","current":true},{"id":"asr","name":"Asr","time":"15:18"},{"id":"maghrib","name":"Maghrib","time":"17:00"},{"id":"isha","name":"Isha","time":"18:20"}],"sunrise":"07:16","sleep":{"time":"23:00"},"night_start":"23:00","blocks":[{"start":"night","end":"fajr","items":[{"id":"n1","title":"Tahajjud","block":"night","repeat":"daily","weekdays":null,"measure":"check","target":null,"position":0,"state":"open","minutes":3,"count":0,"timer_started_at":"2026-11-22T08:00:00Z"}]},{"start":"fajr","end":"dhuhr","items":[{"id":"a1","title":"Read","block":"fajr","repeat":"daily","weekdays":null,"measure":"minutes","target":20,"position":0,"state":"done","minutes":15,"count":0,"timer_started_at":null}]},{"start":"dhuhr","end":"asr","items":[]},{"start":"asr","end":"maghrib","items":[]},{"start":"maghrib","end":"isha","items":[]},{"start":"isha","end":"sleep","items":[]}],"open":1}'
elif [ "$1" = "stats" ]; then
 printf '%s\n' '{"from":"2026-11-09","to":"2026-11-22","days":14,"items":[{"id":"a1","title":"Read","block":"fajr","repeat":"daily","weekdays":null,"measure":"minutes","target":20,"planned_days":14,"done_days":9,"minutes":180,"count":0,"series":[{"day":"2026-11-22","planned":true,"done":true,"minutes":25,"count":0}]}],"prayers":[{"prayer":"fajr","marked_days":10,"days":14}],"marked_total":10,"prayer_slots":70,"daily":[{"day":"2026-11-22","planned":1,"done":1,"minutes":25,"count":0,"prayers":4}],"totals":{"planned":14,"done":9,"minutes":180,"count":0,"prayers":10,"active_days":11}}'
elif [ "$1 $2" = "config show" ]; then printf '%s\n' '{' '  "location": "Tashkent",' '  "method": "uzbekistan",' '  "sleep_time": "23:00"' '}'
elif [ "$1 $2" = "item parse" ]; then printf '%s\n' '{"title":"Read","block":"isha","repeat":"daily","weekdays":null,"measure":"minutes","target":20}'
else printf '%s\n' '{}'; fi
)SH";
        write(path,script); QFile::setPermissions(path,QFile::ReadOwner|QFile::WriteOwner|QFile::ExeOwner); return path;
    }
    static QStringList logLines(QTemporaryDir &dir) { QFile f(dir.filePath("args")); if (!f.open(QIODevice::ReadOnly)) return {}; return QString::fromUtf8(f.readAll()).split('\n',Qt::SkipEmptyParts); }
private slots:
    void asyncQueueAndModels() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir)); QSignalSpy spy(&b,&Backend::requestFinished);
        b.reload(); QTRY_COMPARE_WITH_TIMEOUT(spy.size(),3,3000);
        QCOMPARE(b.todayDay(),QString("2026-11-22")); QCOMPARE(b.dayPlan().value("prayers").toList().size(),5);
        QCOMPARE(b.routinesStats().value("items").toList().first().toMap().value("title").toString(),QString("Read"));
        QCOMPARE(b.config().value("location").toString(),QString("Tashkent"));
        QCOMPARE(logLines(dir).mid(0,3),QStringList({"day --json","stats --days 14 --json","config show"}));
    }
    void timeoutReportsAndQueueContinues() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir),100); QSignalSpy spy(&b,&Backend::requestFinished);
        b.parseQuick("slow"); b.reload(); QTRY_VERIFY_WITH_TIMEOUT(spy.size()>=4,3000); QVERIFY(b.error().contains("timed out"));
    }
    void exactWriteArguments() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir)); QSignalSpy spy(&b,&Backend::requestFinished);
        b.reload(); QTRY_COMPARE(spy.size(),3); b.addQuick("Read Quran after Isha","asr");
        QTRY_VERIFY_WITH_TIMEOUT(logLines(dir).contains("item add --json --quick --block asr -- Read Quran after Isha"),2000);
        b.moveItem("a1","isha",2); b.togglePrayer("fajr",false);
        QTRY_VERIFY_WITH_TIMEOUT(logLines(dir).contains("item move --json a1 --block isha --index 2"),3000);
        QTRY_VERIFY_WITH_TIMEOUT(logLines(dir).contains("prayer mark --json fajr --date 2026-11-22"),3000);
    }
    void rowMoveDestinationAcrossAndEmptyBlocks() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir));
        QVariantList rows{
            QVariantMap{{"kind","header"},{"block","night"}},
            QVariantMap{{"kind","item"},{"block","night"}},
            QVariantMap{{"kind","add"},{"block","night"}},
            QVariantMap{{"kind","header"},{"block","fajr"}},
            QVariantMap{{"kind","item"},{"block","fajr"}},
            QVariantMap{{"kind","item"},{"block","fajr"}},
            QVariantMap{{"kind","add"},{"block","fajr"}},
            QVariantMap{{"kind","header"},{"block","dhuhr"}},
            QVariantMap{{"kind","add"},{"block","dhuhr"}}};
        QCOMPARE(b.moveDestination(rows,6),QVariantMap({{"block","fajr"},{"index",2}}));
        QCOMPARE(b.moveDestination(rows,8),QVariantMap({{"block","dhuhr"},{"index",0}}));
    }
    void unchangedJsonDoesNotResetDayModel() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir)); QSignalSpy spy(&b,&Backend::requestFinished);
        b.reload(); QTRY_COMPARE(spy.size(),3); const int revision=b.dayRevision();
        b.reload(); QTRY_COMPARE(spy.size(),6); QCOMPARE(b.dayRevision(),revision);
    }
    void optimisticTickChangesBeforeCliReturns() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir)); QSignalSpy spy(&b,&Backend::requestFinished);
        b.reload(); QTRY_COMPARE(spy.size(),3); b.toggleDone("a1","done");
        const auto blocks=b.dayPlan().value("blocks").toList();
        QCOMPARE(blocks.at(1).toMap().value("items").toList().first().toMap().value("state").toString(),QString("open"));
        QVERIFY(b.busy());
    }
    void adjustmentArgumentsAndClamp() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir)); QSignalSpy spy(&b,&Backend::requestFinished);
        b.reload(); QTRY_COMPARE(spy.size(),3);
        b.adjustMinutes("a1",-10); b.adjustMinutes("a1",5); b.adjustCount("n1",1); b.adjustCount("n1",-1);
        QTRY_VERIFY_WITH_TIMEOUT(logLines(dir).contains("item log --json --minutes a1 -10"),3000);
        QTRY_VERIFY_WITH_TIMEOUT(logLines(dir).contains("item log --json --minutes a1 5"),3000);
        QTRY_VERIFY_WITH_TIMEOUT(logLines(dir).contains("item log --json n1 1"),3000);
        QTRY_VERIFY_WITH_TIMEOUT(logLines(dir).contains("item log --json n1 -1"),3000);
        b.selectDay("2026-11-23");
        b.adjustMinutes("a1",5); b.adjustMinutes("a1",-5);
        QTRY_VERIFY_WITH_TIMEOUT(logLines(dir).contains("item log --json --minutes a1 5 --date 2026-11-23"),3000);
        QTRY_VERIFY_WITH_TIMEOUT(logLines(dir).contains("item log --json --minutes a1 -5 --date 2026-11-23"),3000);
    }
    void timerStartStopsAnotherOptimistically() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir)); QSignalSpy spy(&b,&Backend::requestFinished);
        b.reload(); QTRY_COMPARE(spy.size(),3); b.toggleTimer("a1",false);
        const auto blocks=b.dayPlan().value("blocks").toList();
        QVERIFY(blocks.at(0).toMap().value("items").toList().first().toMap().value("timer_started_at").toString().isEmpty());
        QVERIFY(!blocks.at(1).toMap().value("items").toList().first().toMap().value("timer_started_at").toString().isEmpty());
        b.toggleTimer("a1",true);
        QTRY_VERIFY_WITH_TIMEOUT(logLines(dir).contains("item stop --json a1"),3000);
    }
    void escapeFunctionClosesTopOverlay() {
        QTemporaryDir dir; Backend backend(nullptr,fake(dir)); Theme theme(dir.path());
        QQmlEngine engine; engine.rootContext()->setContextProperty("backend",&backend); engine.rootContext()->setContextProperty("theme",&theme);
        QQmlComponent component(&engine,QUrl::fromLocalFile(QStringLiteral(TEST_SOURCE_DIR "/../src/Main.qml")));
        QScopedPointer<QObject> root(component.create());
        QVERIFY2(root, qPrintable(component.errorString())); root->setProperty("visible",false);
        root->setProperty("quickVisible",true); root->setProperty("helpVisible",true);
        QVariant result; QVERIFY(QMetaObject::invokeMethod(root.data(),"closeTopOverlay",Q_RETURN_ARG(QVariant,result))); QVERIFY(result.toBool());
        QCOMPARE(root->property("helpVisible").toBool(),false); QCOMPARE(root->property("quickVisible").toBool(),true);
        QVERIFY(QMetaObject::invokeMethod(root.data(),"closeTopOverlay",Q_RETURN_ARG(QVariant,result)));
        QCOMPARE(root->property("quickVisible").toBool(),false);
        QVERIFY(QMetaObject::invokeMethod(root.data(),"closeTopOverlay",Q_RETURN_ARG(QVariant,result))); QVERIFY(!result.toBool());
    }
    void weeklyGuardAndDefaultDay() {
        QTemporaryDir dir; Backend backend(nullptr,fake(dir)); Theme theme(dir.path());
        backend.reload(); QSignalSpy spy(&backend,&Backend::requestFinished); QTRY_VERIFY_WITH_TIMEOUT(spy.size()>=3,3000);
        QQmlEngine engine; engine.rootContext()->setContextProperty("backend",&backend); engine.rootContext()->setContextProperty("theme",&theme);
        QQmlComponent component(&engine,QUrl::fromLocalFile(QStringLiteral(TEST_SOURCE_DIR "/../src/Main.qml")));
        QScopedPointer<QObject> root(component.create()); QVERIFY2(root,qPrintable(component.errorString())); root->setProperty("visible",false);
        QObject *entry=root->findChild<QObject *>("entryPopup"); QVERIFY(entry);
        entry->setProperty("days",QVariantList{});
        QVERIFY(QMetaObject::invokeMethod(entry,"selectRepeat",Q_ARG(QVariant,QVariant("weekly"))));
        QCOMPARE(entry->property("days").toList(),QVariantList({"sun"})); QVERIFY(entry->property("weeklyValid").toBool());
        entry->setProperty("days",QVariantList{}); QVERIFY(!entry->property("weeklyValid").toBool());
    }
    void hijriHeaderFormatting() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir)); const QString h=b.hijriHeader("2026-11-22");
        QVERIFY(h.startsWith("Sunday ")); QVERIFY(h.contains("Jumada")); QCOMPARE(b.gregorianHeader("2026-11-22"),QString("November 22, 2026"));
    }
    void statsRangeQueryAndModels() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir)); QSignalSpy spy(&b,&Backend::requestFinished);
        b.loadStatsRange("2026-11-01","2026-11-30"); QTRY_COMPARE(spy.size(),1);
        QCOMPARE(logLines(dir).last(),QString("stats --from 2026-11-01 --to 2026-11-30 --json"));
        QCOMPARE(b.stats().value("daily").toList().first().toMap().value("minutes").toInt(),25);
        QCOMPARE(b.stats().value("totals").toMap().value("active_days").toInt(),11);
    }
    void statsRangeMathAndTooltip() {
        QTemporaryDir dir; Backend backend(nullptr,fake(dir)); Theme theme(dir.path());
        QQmlEngine engine; engine.rootContext()->setContextProperty("backend",&backend); engine.rootContext()->setContextProperty("theme",&theme);
        QQmlComponent component(&engine,QUrl::fromLocalFile(QStringLiteral(TEST_SOURCE_DIR "/../src/Main.qml")));
        QScopedPointer<QObject> root(component.create()); QVERIFY2(root,qPrintable(component.errorString())); root->setProperty("visible",false);
        QVariant bounds; QVERIFY(QMetaObject::invokeMethod(root.data(),"statsBounds",Q_RETURN_ARG(QVariant,bounds),Q_ARG(QVariant,"week"),Q_ARG(QVariant,QDateTime(QDate(2026,11,22),QTime())),Q_ARG(QVariant,QDateTime()),Q_ARG(QVariant,QDateTime())));
        QCOMPARE(bounds.toMap().value("from").toString(),QString("2026-11-16")); QCOMPARE(bounds.toMap().value("to").toString(),QString("2026-11-22"));
        QVERIFY(QMetaObject::invokeMethod(root.data(),"statsBounds",Q_RETURN_ARG(QVariant,bounds),Q_ARG(QVariant,"month"),Q_ARG(QVariant,QDateTime(QDate(2026,2,15),QTime())),Q_ARG(QVariant,QDateTime()),Q_ARG(QVariant,QDateTime())));
        QCOMPARE(bounds.toMap().value("from").toString(),QString("2026-02-01")); QCOMPARE(bounds.toMap().value("to").toString(),QString("2026-02-28"));
        QVERIFY(QMetaObject::invokeMethod(root.data(),"statsBounds",Q_RETURN_ARG(QVariant,bounds),Q_ARG(QVariant,"custom"),Q_ARG(QVariant,QDateTime()),Q_ARG(QVariant,QDateTime(QDate(2026,12,2),QTime())),Q_ARG(QVariant,QDateTime(QDate(2026,11,29),QTime()))));
        QCOMPARE(bounds.toMap().value("from").toString(),QString("2026-11-29")); QCOMPARE(bounds.toMap().value("to").toString(),QString("2026-12-02"));
        root->setProperty("statsMode","month"); root->setProperty("statsAnchor",QDateTime(QDate(2026,1,31),QTime()));
        QVERIFY(QMetaObject::invokeMethod(root.data(),"stepStats",Q_ARG(QVariant,1))); QCOMPARE(root->property("statsAnchor").toDateTime().date(),QDate(2026,2,1));
        root->setProperty("statsMode","week"); root->setProperty("statsAnchor",QDateTime(QDate(2026,11,22),QTime()));
        QVERIFY(QMetaObject::invokeMethod(root.data(),"stepStats",Q_ARG(QVariant,-1))); QCOMPARE(root->property("statsAnchor").toDateTime().date(),QDate(2026,11,15));
        root->setProperty("statsMode","custom"); root->setProperty("customFrom",QDateTime(QDate(2026,11,29),QTime())); root->setProperty("customTo",QDateTime(QDate(2026,12,2),QTime()));
        QVERIFY(QMetaObject::invokeMethod(root.data(),"stepStats",Q_ARG(QVariant,1))); QCOMPARE(root->property("customFrom").toDateTime().date(),QDate(2026,12,3)); QCOMPARE(root->property("customTo").toDateTime().date(),QDate(2026,12,6));
        QVariant tip; QVariantMap day{{"day","2026-11-18"},{"done",3},{"planned",4},{"minutes",95},{"count",30},{"prayers",4}};
        QVERIFY(QMetaObject::invokeMethod(root.data(),"tooltipText",Q_RETURN_ARG(QVariant,tip),Q_ARG(QVariant,day)));
        QCOMPARE(tip.toString(),QString("Wed 18 Nov · 3 / 4 done · 95 min · 30 counts · 4 prayers"));
    }
    void themeFallbackAndAccent() {
        QTemporaryDir dir; Theme fallback(dir.path()); QCOMPARE(fallback.accent(),QString("#FFD60A")); QCOMPARE(fallback.accentForeground(),QString("black"));
        QDir(dir.path()).mkpath("theme"); write(dir.filePath("theme/colors.toml"),"accent = \"#112233\"\n"); Theme themed(dir.path()); QCOMPARE(themed.accent(),QString("#112233")); QCOMPARE(themed.accentForeground(),QString("white"));
    }
    void translatorFallback() {
        QTemporaryDir dir; Translator t(dir.filePath("language"),dir.path()); QCOMPARE(t.language(),QString("en")); QVERIFY(t.translate("Main","Missing").isNull());
        write(dir.filePath("language"),"pt-BR\n"); write(dir.filePath("en.json"),R"({"Save":"Save"})"); write(dir.filePath("pt.json"),R"({"Save":"Guardar"})"); Translator pt(dir.filePath("language"),dir.path()); QCOMPARE(pt.translate("Main","Save"),QString("Guardar"));
    }
};
QTEST_MAIN(SalahAppTests)
#include "salah_app_tests.moc"
