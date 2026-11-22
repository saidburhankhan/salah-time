QT += core gui qml quick quickcontrols2
CONFIG += c++17 release
TARGET = salah-app
TEMPLATE = app
HEADERS += src/backend.h src/client/coreclient.h src/client/dto.h src/theme.h src/translator.h
SOURCES += src/main.cpp src/backend.cpp src/client/coreclient.cpp src/client/dto.cpp src/theme.cpp src/translator.cpp
RESOURCES += src/resources.qrc
DISTFILES += $$files(src/qml/*.qml, true)
