// Weekly review: the computed summary and the optional reflection
// (review.get / review.save).
#include "backend.h"

#include <QDate>
#include <QJsonObject>

namespace {
// The Monday of the week holding `day`, or an invalid date.
QDate mondayOf(const QString &day) {
    const QDate date = QDate::fromString(day, Qt::ISODate);
    return date.isValid() ? date.addDays(1 - date.dayOfWeek()) : QDate();
}
} // namespace

QString Backend::reviewWeekStart(const QString &day, int offsetWeeks) const {
    const QDate monday = mondayOf(day.isEmpty() ? m_today.toString(Qt::ISODate) : day);
    return monday.isValid() ? monday.addDays(7 * offsetWeeks).toString(Qt::ISODate) : QString();
}

void Backend::loadReview(const QString &week) {
    const QString monday = mondayOf(week).toString(Qt::ISODate);
    const quint64 request = ++m_reviewRequest;
    if (monday.isEmpty() || m_client.offline()) {
        m_review.clear();
        emit reviewChanged();
        return;
    }
    if (m_review.value(QStringLiteral("week")).toString() != monday) {
        m_review.clear();
        emit reviewChanged();
    }
    m_reviewWeek = monday;
    m_client.rpcRead(QStringLiteral("review.get"), QJsonObject{{QStringLiteral("week"), monday}},
                     [this, request](const CoreClient::Result &result) {
        if (request != m_reviewRequest) return;
        if (result.ok) {
            m_review = std::get<Dto::Json>(result.value).json.toVariantMap();
        } else {
            m_review.clear();
            if (!m_client.offline()) fail(result.error.message);
        }
        emit reviewChanged();
    });
}

void Backend::searchReviewNotes(int request, const QString &week, const QString &query, const QString &after) {
    QJsonObject params{{"week", week}, {"query", query}};
    if (!after.isEmpty()) params.insert("after", after);
    m_client.rpcRead(QStringLiteral("review.notes"), params, [this, request](const CoreClient::Result &result) {
        if (const auto *data = std::get_if<Dto::Json>(&result.value); result.ok && data && data->json.value("rows").isArray()) {
            emit reviewNotesLoaded(request, data->json.toVariantMap(), {}, false);
        } else {
            emit reviewNotesLoaded(request, {}, result.ok ? tr("Could not read matching notes.") : result.error.message,
                                   result.error.rpcCode == -32601);
        }
    });
}

void Backend::saveReview(const QString &week, const QString &note) {
    const QString monday = mondayOf(week).toString(Qt::ISODate);
    if (monday.isEmpty()) return;
    // An empty note removes the reflection.
    const QString text = note.trimmed().isEmpty() ? QString() : note;
    auto &operations = m_reviewSaveOperations[monday];
    QString &pending = operations[text];
    if (pending.isEmpty()) pending = CoreClient::makeOpId();
    const QString operation = pending;
    m_client.mutateEntity(QStringLiteral("review.save"),
                          QJsonObject{{QStringLiteral("week"), monday}, {QStringLiteral("note"), text}},
                          [this, monday, note, text, operation](const CoreClient::Result &result) {
        if (result.ok || result.error.definitiveRejection()) {
            auto week = m_reviewSaveOperations.find(monday);
            if (week != m_reviewSaveOperations.end() && week->value(text) == operation) {
                week->remove(text);
                if (week->isEmpty()) m_reviewSaveOperations.erase(week);
            }
        }
        if (!result.ok) {
            fail(result.error.message);
            emit reviewSaveFinished(false, result.error.message, monday, note);
            return;
        }
        clearError();
        emit reviewSaveFinished(true, QString(), monday, note);
        if (monday == m_reviewWeek) loadReview(monday);
    }, operation);
}
