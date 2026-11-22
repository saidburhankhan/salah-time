QT += core gui quick quickcontrols2 testlib
DEFINES += TEST_SOURCE_DIR=\\\"$$PWD\\\"
CONFIG += c++17 testcase
TARGET = salah_app_tests
TEMPLATE = app
INCLUDEPATH += ../src
HEADERS += ../src/backend.h ../src/client/coreclient.h ../src/client/dto.h ../src/theme.h ../src/translator.h
SOURCES += salah_app_tests.cpp ../src/backend.cpp ../src/client/coreclient.cpp ../src/client/dto.cpp ../src/theme.cpp ../src/translator.cpp
