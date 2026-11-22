QT += core gui quick quickcontrols2 testlib
CONFIG += c++17 testcase
TARGET = salah_app_tests
TEMPLATE = app
INCLUDEPATH += ../src
HEADERS += ../src/backend.h ../src/theme.h ../src/translator.h
SOURCES += salah_app_tests.cpp ../src/backend.cpp ../src/theme.cpp ../src/translator.cpp
