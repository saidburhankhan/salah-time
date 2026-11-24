// Salah — calm planning and reflection between the prayers.
#include <QGuiApplication>
#include <QIcon>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickStyle>
#include "backend.h"
#include "launch.h"
#include <QTextStream>
#include "theme.h"
#include "translator.h"

int main(int argc, char *argv[]) {
    QGuiApplication app(argc, argv);
    app.setApplicationName("Salah");
    app.setDesktopFileName("salah");
    app.setWindowIcon(QIcon::fromTheme("salah"));
    QQuickStyle::setStyle("Material");
    Translator translator; app.installTranslator(&translator);
    Theme theme; Backend backend;
    QQmlApplicationEngine engine;
    QObject::connect(&translator, &Translator::changed, &engine, &QQmlEngine::retranslate);
    engine.rootContext()->setContextProperty("translator", &translator);
    engine.rootContext()->setContextProperty("theme", &theme);
    engine.rootContext()->setContextProperty("backend", &backend);
    engine.load(QUrl("qrc:/qml/Main.qml"));
    if (engine.rootObjects().isEmpty()) return 1;
    backend.reload();
    QString warning;
    const QString page = launch::startPage(app.arguments(), &warning);
    if (!warning.isEmpty()) QTextStream(stderr) << warning << Qt::endl;
    QMetaObject::invokeMethod(engine.rootObjects().first(), "showPage", Q_ARG(QVariant, page));
    return app.exec();
}
