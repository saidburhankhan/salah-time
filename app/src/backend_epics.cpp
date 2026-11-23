// Backend reads and writes for epics, kept apart from backend.cpp. Epics are
// listed with the other Settings lists (loadSettings); this file adds the
// removal and the data behind the Epic page.
#include "backend.h"

#include <QJsonArray>
#include <QJsonObject>

void Backend::removeEpic(const QString &id) {
    if (id.isEmpty()) return;
    m_client.mutateEntity(QStringLiteral("epic.remove"), QJsonObject{{QStringLiteral("id"), id}},
                          entityResponse(QStringLiteral("epic")));
}

void Backend::loadEpicPage(const QString &epicId) {
    const quint64 request = ++m_epicRequest;
    m_epicSeries.clear();
    m_epicSessions.clear();
    emit epicPageChanged();
    // Both reads are daemon-only; offline the page shows its totals only.
    if (epicId.isEmpty() || m_client.offline()) return;
    QJsonObject series{{QStringLiteral("days"), 90}, {QStringLiteral("epic_id"), epicId},
                       {QStringLiteral("interval"), QStringLiteral("day")},
                       {QStringLiteral("measures"), QJsonArray{QStringLiteral("minutes")}}};
    m_client.statsSeries(series, [this, request](const CoreClient::Result &result) {
        if (request != m_epicRequest || !result.ok) return;
        if (const auto *json = std::get_if<Dto::Json>(&result.value)) {
            m_epicSeries = json->json.toVariantMap();
            emit epicPageChanged();
        }
    });
    const QJsonObject sessions{{QStringLiteral("days"), 730}, {QStringLiteral("epic_id"), epicId},
                               {QStringLiteral("with_note"), true}, {QStringLiteral("limit"), 50}};
    m_client.rpcRead(QStringLiteral("sessions.list"), sessions, [this, request](const CoreClient::Result &result) {
        if (request != m_epicRequest || !result.ok) return;
        if (const auto *json = std::get_if<Dto::Json>(&result.value)) {
            m_epicSessions = json->json.value(QStringLiteral("rows")).toArray().toVariantList();
            emit epicPageChanged();
        }
    });
}
