QT += core gui quick quickcontrols2 network testlib
DEFINES += TEST_SOURCE_DIR=\\\"$$PWD\\\"
CONFIG += c++17 testcase
TARGET = salah_app_tests
TEMPLATE = app
INCLUDEPATH += ../src
HEADERS += ../src/backend.h ../src/models/typedlistmodel.h ../src/models/daytimelinemodel.h ../src/models/prayermodel.h ../src/models/statsseriesmodel.h ../src/models/statsitemsmodel.h ../src/models/routinesmodel.h ../src/models/statsdailymodel.h ../src/models/statsticksmodel.h ../src/models/viewstate.h ../src/client/coreclient.h ../src/client/sockettransport.h ../src/client/dto.h ../src/theme.h ../src/translator.h
SOURCES += salah_app_tests.cpp ../src/backend.cpp ../src/models/typedlistmodel.cpp ../src/models/daytimelinemodel.cpp ../src/models/prayermodel.cpp ../src/models/statsseriesmodel.cpp ../src/models/statsitemsmodel.cpp ../src/models/routinesmodel.cpp ../src/models/statsdailymodel.cpp ../src/models/statsticksmodel.cpp ../src/models/viewstate.cpp ../src/client/coreclient.cpp ../src/client/sockettransport.cpp ../src/client/dto.cpp ../src/theme.cpp ../src/translator.cpp
RESOURCES += ../src/resources.qrc
