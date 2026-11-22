#include <QDir>
#include <QFile>
#include <QTemporaryDir>
#include <QtTest>
#include "backend.h"
#include "theme.h"
#include "translator.h"

class SalahAppTests : public QObject {
    Q_OBJECT
private:
    static void write(const QString &path, const QByteArray &data) {
        QFile f(path); QVERIFY(f.open(QIODevice::WriteOnly)); QCOMPARE(f.write(data), data.size()); f.close();
    }
    static QString fake(QTemporaryDir &dir, bool failing = false) {
        const QString path=dir.filePath("salah"), log=dir.filePath("args");
        QByteArray script = "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '"+log.toUtf8()+"'\n";
        if (failing) script += "echo 'fixture failure' >&2\nexit 7\n";
        else script += R"SH(
if [ "$1 $2 $3" = "item list --json" ]; then
  printf '%s\n' '{"day":"2026-11-22","block":"dhuhr","planning_block":{"day":"2026-11-22","start":"asr"},"current":[],"open":0,"items":[],"upcoming":[]}'
elif [ "$1" = "history" ]; then
  printf '%s\n' '[{"day":"2026-11-22","items":[{"id":"a1","title":"Read","block":"fajr","repeat":"daily","measure":"minutes","target":20,"state":"done","minutes":15,"count":0,"timer_started_at":null}]}]'
else
  printf '%s\n' '{}'
fi
)SH";
        write(path,script); QFile::setPermissions(path,QFile::ReadOwner|QFile::WriteOwner|QFile::ExeOwner); return path;
    }
private slots:
    void parsesTodayAndHistory() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir)); b.reload();
        QCOMPARE(b.todayDay(),QString("2026-11-22")); QCOMPARE(b.planningBlock(),QString("asr"));
        QCOMPARE(b.selectedItems().size(),1); QCOMPARE(b.selectedItems().first().toMap().value("title").toString(),QString("Read"));
        QCOMPARE(b.routines().size(),1);
    }
    void cliFailureSurfaces() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir,true)); b.reload(); QCOMPARE(b.error(),QString("fixture failure"));
    }
    void addBuildsExactArguments() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir)); b.addItem("Read Quran","asr",true,"minutes",30);
        QFile f(dir.filePath("args")); QVERIFY(f.open(QIODevice::ReadOnly));
        QCOMPARE(QString::fromUtf8(f.readLine()).trimmed(),QString("item add --json --block asr --daily --measure minutes --target 30 -- Read Quran"));
    }
    void doneBuildsExactArguments() {
        QTemporaryDir dir; Backend b(nullptr,fake(dir)); b.reload(); b.toggleDone("a1","open");
        QFile f(dir.filePath("args")); QVERIFY(f.open(QIODevice::ReadOnly)); QStringList lines=QString::fromUtf8(f.readAll()).split('\n',Qt::SkipEmptyParts);
        QVERIFY(lines.contains("item done --json a1"));
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
