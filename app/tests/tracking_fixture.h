#pragma once

// A realistic tracking day for UI tests and screenshots: projects with
// colours and tags, a running session, a ticked item, timed items, and a
// month of statistics. Shapes follow crates/salah-cli/tests/fixtures/rpc/.

#include <QDateTime>
#include <QJsonArray>
#include <QJsonObject>
#include <QLocale>
#include <QStringList>
#include <QtMath>

namespace TrackingFixture {

inline const char *kDay = "2031-05-14";

inline QJsonObject metric(const QString &id, const QString &name, const QString &unit,
                          const QString &kind, int scaleMax, const QString &rollup,
                          bool builtin, bool archived = false) {
    return QJsonObject{
        {"id", id}, {"name", name}, {"unit", unit}, {"kind", kind},
        {"scale_max", scaleMax > 0 ? QJsonValue(scaleMax) : QJsonValue(QJsonValue::Null)},
        {"rollup", rollup}, {"builtin", builtin}, {"archived", archived},
        {"created_at", "2031-05-01T04:30:00Z"}, {"updated_at", "2031-05-01T04:30:00Z"},
        {"deleted_at", QJsonValue::Null}, {"version", 1}};
}

inline QJsonArray metrics() {
    return QJsonArray{
        metric("focus", "Focus", "", "scale", 5, "avg", true),
        metric("quality", "Quality", "", "scale", 5, "avg", true),
        metric("pages", "Pages read", "pages", "count", 0, "sum", false),
        metric("memorised", "Memorised", "", "check", 0, "sum", false),
        metric("reps", "Push-ups", "reps", "count", 0, "sum", false),
        metric("old", "Old mood", "", "scale", 3, "avg", false, true),
        metric("distractions", "Distractions", "times", "count", 0, "sum", true)};
}

inline QJsonObject metricById(const QString &id) {
    for (const QJsonValue &value : metrics())
        if (value.toObject().value("id").toString() == id) return value.toObject();
    return {};
}

inline QJsonObject tag(const QString &id, const QString &name, const QString &color) {
    return QJsonObject{{"id", id}, {"name", name}, {"color", color},
                       {"created_at", "2031-05-01T04:30:00Z"}, {"updated_at", "2031-05-01T04:30:00Z"},
                       {"deleted_at", QJsonValue::Null}, {"version", 1}};
}

inline QJsonArray tags() {
    return QJsonArray{tag("faith", "Faith", "#2e7d32"), tag("health", "Health", "#e0663f"),
                      tag("learning", "Learning", "#9b59b6")};
}

struct Link { QString metric; bool required; };

inline QJsonObject project(const QString &id, const QString &name, const QString &color,
                           const QString &note, const QStringList &tagIds,
                           const QList<Link> &links, bool archived = false) {
    QJsonArray tagRows, tagIdRows, metricRows;
    for (const QString &tagId : tagIds) {
        tagIdRows.append(tagId);
        for (const QJsonValue &t : tags())
            if (t.toObject().value("id").toString() == tagId) tagRows.append(t);
    }
    int position = 0;
    for (const Link &link : links)
        metricRows.append(QJsonObject{{"metric", metricById(link.metric)}, {"metric_id", link.metric},
                                      {"position", position++}, {"project_id", id},
                                      {"required", link.required}});
    return QJsonObject{{"id", id}, {"name", name}, {"color", color},
                       {"note", note.isEmpty() ? QJsonValue(QJsonValue::Null) : QJsonValue(note)},
                       {"archived", archived}, {"tag_ids", tagIdRows}, {"tags", tagRows},
                       {"metrics", metricRows}, {"created_at", "2031-05-01T04:30:00Z"},
                       {"updated_at", "2031-05-01T04:30:00Z"}, {"deleted_at", QJsonValue::Null},
                       {"version", 1}};
}

inline QJsonArray projects() {
    return QJsonArray{
        project("quran", "Quran", "#4c8bf5", "Daily recitation", {"faith"},
                {{"focus", true}, {"quality", true}, {"pages", false}, {"memorised", false}}),
        project("study", "Study", "#9b59b6", "Arabic grammar", {"learning"},
                {{"focus", true}, {"quality", false}, {"distractions", false}}),
        project("fitness", "Fitness", "#e0663f", "", {"health"},
                {{"reps", true}, {"focus", false}}),
        project("garden", "Garden", "#26a69a", "", {}, {}, true)};
}

inline QJsonObject projectRef(const QString &id) {
    for (const QJsonValue &value : projects()) {
        const QJsonObject p = value.toObject();
        if (p.value("id").toString() == id)
            return QJsonObject{{"id", id}, {"name", p.value("name")}, {"color", p.value("color")}};
    }
    return {};
}

inline QJsonObject session(const QString &id, const QString &itemId, const QString &projectId,
                           const QString &block, int minutes, const QJsonObject &values,
                           const QString &startedAt = {}, bool running = false,
                           const QString &note = {}, const QJsonArray &notes = {}) {
    QJsonValue ended = QJsonValue::Null;
    if (!startedAt.isEmpty() && !running && minutes > 0)
        ended = QDateTime::fromString(startedAt, Qt::ISODate).addSecs(minutes * 60).toString(Qt::ISODate);
    return QJsonObject{
        {"id", id}, {"item_id", itemId.isEmpty() ? QJsonValue(QJsonValue::Null) : QJsonValue(itemId)},
        {"project_id", projectId.isEmpty() ? QJsonValue(QJsonValue::Null) : QJsonValue(projectId)},
        {"block", block}, {"day", kDay},
        {"started_at", startedAt.isEmpty() ? QJsonValue(QJsonValue::Null) : QJsonValue(startedAt)},
        {"ended_at", ended},
        {"minutes", minutes > 0 ? QJsonValue(minutes) : QJsonValue(QJsonValue::Null)},
        {"note", note.isEmpty() ? QJsonValue(QJsonValue::Null) : QJsonValue(note)},
        {"running", running}, {"values", values}, {"notes", notes}, {"version", 1}};
}

// The running session started 47 minutes before "now" so elapsed time shows.
inline QString runningStart() {
    return QDateTime::currentDateTimeUtc().addSecs(-47 * 60).toString(Qt::ISODate);
}

inline QJsonObject runningSession() {
    return session("s-run", "deep", "study", "dhuhr", 0, QJsonObject{{"distractions", 3.0}}, runningStart(), true, {},
                   QJsonArray{QJsonObject{{"id", "n-1"}, {"at", "2031-05-14T07:41:00Z"}, {"text", "Phone buzzed, put it away"}},
                              QJsonObject{{"id", "n-2"}, {"at", "2031-05-14T07:58:00Z"}, {"text", "Verb forms done, starting exercises"}}});
}

inline QJsonObject staleRunningSession() {
    QJsonObject running = runningSession();
    running["started_at"] = QDateTime::currentDateTimeUtc().addSecs(-10 * 3600).toString(Qt::ISODate);
    running["stale"] = true;
    running["max_minutes"] = 120;
    return running;
}

inline QJsonObject item(const QString &id, const QString &title, const QString &block, int position,
                        const QString &projectId, int planned, const QJsonArray &sessions,
                        const QString &repeat = "daily") {
    // Done means the item has a tick; timed sessions only add minutes.
    int actual = 0;
    bool done = false;
    for (const QJsonValue &value : sessions) {
        const QJsonObject s = value.toObject();
        if (s.value("running").toBool()) continue;
        if (s.value("minutes").isNull()) done = true;
        else actual += s.value("minutes").toInt();
    }
    const QString schedule = repeat == "daily" ? "Daily" : "Once";
    const QString detail = planned > 0 ? QString("%1 / %2 min · %3").arg(actual).arg(planned).arg(schedule)
                                       : schedule;
    return QJsonObject{
        {"id", id}, {"title", title}, {"block", block}, {"position", position},
        {"project", projectId.isEmpty() ? QJsonValue(QJsonValue::Null) : QJsonValue(projectRef(projectId))},
        {"planned_minutes", planned > 0 ? QJsonValue(planned) : QJsonValue(QJsonValue::Null)},
        {"actual_minutes", actual}, {"done", done}, {"sessions", sessions}, {"repeat", repeat},
        {"schedule_text", schedule}, {"detail_text", detail}, {"next_occurrence", kDay},
        {"version", 1}, {"weekdays", QJsonValue::Null}};
}

inline QJsonArray dayItems(const QString &block, bool withRunning = true) {
    if (block == "fajr")
        return QJsonArray{
            item("quran", "Read Quran", "fajr", 0, "quran", 30,
                 QJsonArray{session("s-q1", "quran", "quran", "fajr", 25, {},
                                    "2031-05-14T00:30:00Z", false, {},
                                    QJsonArray{QJsonObject{{"id", "n-a"}, {"at", "2031-05-14T00:41:00Z"}, {"text", "Calm morning, slow recitation"}},
                                               QJsonObject{{"id", "n-b"}, {"at", "2031-05-14T00:52:00Z"}, {"text", "Stopped at page 12"}}}),
                            session("s-q2", "quran", "quran", "fajr", 10, {}),
                            session("s-q3", "quran", "quran", "fajr", 0,
                                    QJsonObject{{"focus", 4.0}, {"quality", 5.0}, {"pages", 6.0}},
                                    {}, false, "Finished the juz")}),
            item("walk", "Morning walk", "fajr", 1, "", 0,
                 QJsonArray{session("s-w1", "walk", "", "fajr", 0, {})})};
    if (block == "dhuhr") {
        QJsonArray deep{session("s-d1", "deep", "study", "dhuhr", 40,
                                QJsonObject{{"focus", 4.0}}, "2031-05-14T07:30:00Z")};
        if (withRunning) deep.append(runningSession());
        return QJsonArray{item("deep", "Deep work: grammar", "dhuhr", 0, "study", 90, deep),
                          item("dhikr", "Dhikr after Dhuhr", "dhuhr", 1, "quran", 10, {})};
    }
    if (block == "asr")
        return QJsonArray{item("gym", "Gym session", "asr", 0, "fitness", 45, {})};
    if (block == "isha")
        return QJsonArray{item("review", "Review notes", "isha", 0, "study", 20, {}, "none")};
    return {};
}

inline QJsonObject day(const QJsonObject &base, bool withRunning = true) {
    QJsonObject result = base;
    const QStringList starts{"night", "fajr", "dhuhr", "asr", "maghrib", "isha"};
    const QStringList ends{"fajr", "dhuhr", "asr", "maghrib", "isha", "sleep"};
    const QList<int> capacity{252, 547, 305, 129, 107, 100};
    QJsonArray blocks;
    int plannedItems = 0, doneItems = 0, plannedTotal = 0, actualTotal = 0;
    for (int i = 0; i < starts.size(); ++i) {
        const QJsonArray items = dayItems(starts.at(i), withRunning);
        int planned = 0, actual = 0;
        for (const QJsonValue &value : items) {
            const QJsonObject row = value.toObject();
            planned += row.value("planned_minutes").toInt();
            actual += row.value("actual_minutes").toInt();
            ++plannedItems;
            if (row.value("done").toBool()) ++doneItems;
        }
        plannedTotal += planned;
        actualTotal += actual;
        blocks.append(QJsonObject{{"start", starts.at(i)}, {"end", ends.at(i)}, {"items", items},
                                  {"capacity_minutes", capacity.at(i)}, {"planned_minutes", planned},
                                  {"actual_minutes", actual}, {"unplanned_minutes", 0}});
    }
    result["blocks"] = blocks;
    result["fulfilment"] = QJsonObject{{"planned_items", plannedItems}, {"done_items", doneItems},
                                       {"planned_minutes", plannedTotal}, {"actual_minutes", actualTotal},
                                       {"unplanned_minutes", 15}};
    result["open"] = plannedItems - doneItems;
    // Next prayer 2h 49m from the real clock, so the countdown reads sensibly.
    QJsonObject now = result.value("now").toObject();
    now["block"] = "dhuhr";
    QJsonObject next = now.value("next").toObject();
    next["at"] = QDateTime::currentDateTimeUtc().addSecs(10140).toString(Qt::ISODate);
    next["remaining_seconds"] = 10140;
    now["next"] = next;
    result["now"] = now;
    return result;
}

inline QJsonArray promptFor(const QString &projectId) {
    QJsonArray prompt;
    for (const QJsonValue &value : projects()) {
        const QJsonObject p = value.toObject();
        if (p.value("id").toString() != projectId) continue;
        for (const QJsonValue &linkValue : p.value("metrics").toArray()) {
            const QJsonObject link = linkValue.toObject();
            const QJsonObject m = link.value("metric").toObject();
            prompt.append(QJsonObject{{"metric_id", m.value("id")}, {"name", m.value("name")},
                                      {"unit", m.value("unit")}, {"kind", m.value("kind")},
                                      {"scale_max", m.value("scale_max")},
                                      {"required", link.value("required")}});
        }
    }
    return prompt;
}

inline QJsonObject status(const QJsonObject &base, bool withRunning = true) {
    QJsonObject result = base;
    result["running"] = withRunning ? QJsonValue(runningSession()) : QJsonValue(QJsonValue::Null);
    result["prompt"] = withRunning ? promptFor("study") : QJsonArray{};
    return result;
}

// A 28-day range ending on the fixture day, with a few quiet days.
inline QJsonObject stats(const QString &tagId = {}, const QString &projectId = {}, int dayCount = 28) {
    const QDate last = QDate::fromString(kDay, Qt::ISODate);
    const QDate first = last.addDays(-(dayCount - 1));
    QJsonArray days, minutesPoints, focusPoints, qualityPoints, pagesPoints;
    int totalMinutes = 0, activeDays = 0, sessions = 0;
    const double scale = projectId.isEmpty() && tagId.isEmpty() ? 1.0 : 0.55;
    for (int i = 0; i < dayCount; ++i) {
        const QDate date = first.addDays(i);
        const bool quiet = (i % 7 == 5) || i == 2;
        const int minutes = quiet ? 0 : int((60 + 45 * qSin(i * 0.7) + (i % 4) * 18) * scale);
        const int planned = int(130 * scale);
        totalMinutes += minutes;
        if (minutes > 0) { ++activeDays; sessions += 2 + i % 3; }
        const QString iso = date.toString(Qt::ISODate);
        days.append(QJsonObject{{"day", iso}, {"minutes", minutes}, {"capacity_minutes", 1180 + i % 9},
                                {"planned_minutes", planned}, {"unplanned_minutes", minutes > 100 ? 15 : 0},
                                {"fulfilment_ratio", planned > 0 ? QJsonValue(qMin(1.0, double(minutes) / planned))
                                                                 : QJsonValue(QJsonValue::Null)}});
        minutesPoints.append(QJsonObject{{"day", iso}, {"value", double(minutes)}});
        focusPoints.append(QJsonObject{{"day", iso}, {"value", minutes > 0 ? QJsonValue(3.0 + qSin(i * 0.5) * 1.2)
                                                                           : QJsonValue(QJsonValue::Null)}});
        qualityPoints.append(QJsonObject{{"day", iso}, {"value", minutes > 0 ? QJsonValue(3.6 + qCos(i * 0.4) * 0.9)
                                                                             : QJsonValue(QJsonValue::Null)}});
        pagesPoints.append(QJsonObject{{"day", iso}, {"value", minutes > 0 && i % 2 == 0 ? QJsonValue(double(4 + i % 6))
                                                                                         : QJsonValue(QJsonValue::Null)}});
    }
    QJsonArray series{
        QJsonObject{{"metric_id", "minutes"}, {"name", "Minutes"}, {"unit", "min"}, {"rollup", "sum"}, {"points", minutesPoints}},
        QJsonObject{{"metric_id", "focus"}, {"name", "Focus"}, {"unit", ""}, {"rollup", "avg"}, {"points", focusPoints}},
        QJsonObject{{"metric_id", "quality"}, {"name", "Quality"}, {"unit", ""}, {"rollup", "avg"}, {"points", qualityPoints}},
        QJsonObject{{"metric_id", "pages"}, {"name", "Pages read"}, {"unit", "pages"}, {"rollup", "sum"}, {"points", pagesPoints}}};
    QJsonArray projectRows{
        QJsonObject{{"id", "quran"}, {"name", "Quran"}, {"color", "#4c8bf5"}, {"minutes", int(totalMinutes * 0.45)},
                    {"sessions", sessions / 2}, {"metrics", QJsonObject{{"focus", 3.9}, {"quality", 4.4}, {"pages", 96.0}}}},
        QJsonObject{{"id", "study"}, {"name", "Study"}, {"color", "#9b59b6"}, {"minutes", int(totalMinutes * 0.35)},
                    {"sessions", sessions / 3}, {"metrics", QJsonObject{{"focus", 3.4}}}},
        QJsonObject{{"id", "fitness"}, {"name", "Fitness"}, {"color", "#e0663f"}, {"minutes", int(totalMinutes * 0.2)},
                    {"sessions", sessions / 6}, {"metrics", QJsonObject{{"reps", 420.0}}}}};
    QJsonArray tagRows{
        QJsonObject{{"id", "faith"}, {"name", "Faith"}, {"color", "#2e7d32"}, {"minutes", int(totalMinutes * 0.45)}},
        QJsonObject{{"id", "learning"}, {"name", "Learning"}, {"color", "#9b59b6"}, {"minutes", int(totalMinutes * 0.35)}},
        QJsonObject{{"id", "health"}, {"name", "Health"}, {"color", "#e0663f"}, {"minutes", int(totalMinutes * 0.2)}}};
    QJsonArray blockRows{
        QJsonObject{{"block", "night"}, {"minutes", 0}, {"capacity_minutes", 7056}, {"avg_focus", QJsonValue::Null}},
        QJsonObject{{"block", "fajr"}, {"minutes", int(totalMinutes * 0.4)}, {"capacity_minutes", 15316}, {"avg_focus", 3.9}},
        QJsonObject{{"block", "dhuhr"}, {"minutes", int(totalMinutes * 0.35)}, {"capacity_minutes", 8540}, {"avg_focus", 3.3}},
        QJsonObject{{"block", "asr"}, {"minutes", int(totalMinutes * 0.2)}, {"capacity_minutes", 3612}, {"avg_focus", 3.6}},
        QJsonObject{{"block", "maghrib"}, {"minutes", 0}, {"capacity_minutes", 2996}, {"avg_focus", QJsonValue::Null}},
        QJsonObject{{"block", "isha"}, {"minutes", int(totalMinutes * 0.05)}, {"capacity_minutes", 2800}, {"avg_focus", 2.8}}};
    return QJsonObject{
        {"range", QJsonObject{{"from", first.toString(Qt::ISODate)}, {"to", last.toString(Qt::ISODate)}}},
        {"filter", QJsonObject{{"tag_id", tagId.isEmpty() ? QJsonValue(QJsonValue::Null) : QJsonValue(tagId)},
                               {"project_id", projectId.isEmpty() ? QJsonValue(QJsonValue::Null) : QJsonValue(projectId)}}},
        {"days", days}, {"series", series}, {"projects", projectRows}, {"tags", tagRows}, {"blocks", blockRows},
        {"totals", QJsonObject{{"minutes", totalMinutes}, {"sessions", sessions}, {"active_days", activeDays},
                               {"avg_focus", 3.6}, {"avg_quality", 4.1}}}};
}

// A 28-day stats.series response for the Trends screenshots. Each task has a
// different sampling pattern, including sessions without a Focus rating.
inline QJsonObject trendMonth(const QJsonArray &measures, bool tasks) {
    const QDate first = QDate::fromString(kDay, Qt::ISODate).addDays(-27);
    QJsonArray periods, lines;
    for (int d = 0; d < 28; ++d) periods.append(first.addDays(d).toString(Qt::ISODate));
    const QStringList titles{"Read Quran", "Deep work", "Morning walk"};
    for (int task = 0; task < (tasks ? titles.size() : 1); ++task) {
        for (const auto &measure : measures) {
            const bool minutes = measure.toString() == "minutes";
            const QString name = minutes ? "Minutes" : "Focus";
            QJsonArray points;
            for (int d = 0; d < 28; ++d) {
                const int sessions = (d + task * 2) % 9 == 5 ? 0 : 1 + d % 3;
                QJsonValue value;
                if (minutes)
                    value = sessions ? 25 + (d * 7) % 60 : 0;
                else if (sessions && (d + task) % 13 != 6)
                    value = qRound(10 * (2.9 + 0.4 * task + 0.9 * qSin(0.38 * d + 2 * task))) / 10.0;
                points.append(QJsonObject{{"value", value}, {"sessions", sessions}});
            }
            lines.append(QJsonObject{{"key", tasks ? QString::number(task) : QStringLiteral("all")},
                                     {"label", tasks ? titles[task] : name}, {"color", QJsonValue::Null},
                                     {"measure", measure}, {"unit", minutes ? "min" : ""},
                                     {"scale_max", minutes ? QJsonValue(QJsonValue::Null) : QJsonValue(5)},
                                     {"points", points}});
        }
    }
    return QJsonObject{{"periods", periods}, {"series", lines}, {"interval", "day"},
                       {"split", tasks ? QJsonValue("item") : QJsonValue(QJsonValue::Null)}};
}

inline QJsonObject tableColumn(const QString &id, const QString &name, const QString &unit, const QString &kind,
                               const QString &rollup, int scaleMax = 0, const QString &derivedFrom = {}) {
    return QJsonObject{{"id", id}, {"name", name}, {"unit", unit}, {"kind", kind}, {"rollup", rollup},
                       {"scale_max", scaleMax > 0 ? QJsonValue(scaleMax) : QJsonValue(QJsonValue::Null)},
                       {"archived", false},
                       {"derived_from", derivedFrom.isEmpty() ? QJsonValue(QJsonValue::Null) : QJsonValue(derivedFrom)}};
}

inline QJsonValue maybe(double value) { return value < 0 ? QJsonValue(QJsonValue::Null) : QJsonValue(value); }

// One stats.table row; negative metric values are null (no sample).
inline QJsonObject tableRow(const QString &key, const QString &label, const QString &color, const QString &from,
                            const QString &to, int minutes, int sessions, double focus, double quality,
                            double pages, double distractions) {
    const double perHour = distractions < 0 || minutes <= 0 ? -1 : distractions * 60.0 / minutes;
    return QJsonObject{{"key", key}, {"label", label},
                       {"color", color.isEmpty() ? QJsonValue(QJsonValue::Null) : QJsonValue(color)},
                       {"from", from.isEmpty() ? QJsonValue(QJsonValue::Null) : QJsonValue(from)},
                       {"to", to.isEmpty() ? QJsonValue(QJsonValue::Null) : QJsonValue(to)},
                       {"minutes", minutes}, {"sessions", sessions},
                       {"values", QJsonObject{{"focus", maybe(focus)}, {"quality", maybe(quality)},
                                              {"pages", maybe(pages)}, {"distractions", maybe(distractions)},
                                              {"distractions_per_hour", maybe(perHour)}}}};
}

// stats.table in the shape of stats_table_week.json / stats_table_project.json
// for the 28 days ending on the fixture day.
inline QJsonObject statsTable(const QString &group) {
    QJsonArray rows;
    if (group == "project") {
        rows = {tableRow("quran", "Quran", "#4c8bf5", {}, {}, 812, 31, 3.9, 4.4, 96, 4),
                tableRow("study", "Study", "#9b59b6", {}, {}, 640, 18, 3.2, -1, -1, 21),
                tableRow("fitness", "Fitness", "#e0663f", {}, {}, 365, 12, 2.4, -1, -1, -1),
                tableRow("none", "No project", {}, {}, {}, 95, 6, -1, -1, -1, -1)};
    } else if (group == "tag") {
        rows = {tableRow("faith", "Faith", "#2e7d32", {}, {}, 812, 31, 3.9, 4.4, 96, 4),
                tableRow("learning", "Learning", "#9b59b6", {}, {}, 640, 18, 3.2, -1, -1, 21),
                tableRow("health", "Health", "#e0663f", {}, {}, 365, 12, 2.4, -1, -1, -1),
                tableRow("none", "No tag", {}, {}, {}, 95, 6, -1, -1, -1, -1)};
    } else if (group == "item") {
        rows = {tableRow("quran", "Read Quran", "#4c8bf5", {}, {}, 790, 30, 3.9, 4.4, 96, 4),
                tableRow("deep", "Deep work: grammar", "#9b59b6", {}, {}, 600, 16, 3.1, -1, -1, 20),
                tableRow("gym", "Gym session", "#e0663f", {}, {}, 365, 12, 2.4, -1, -1, -1),
                tableRow("review", "Review notes", "#9b59b6", {}, {}, 40, 2, 4.5, -1, -1, 1),
                tableRow("none", "No task", {}, {}, {}, 117, 7, 1.0, -1, -1, -1)};
    } else if (group == "day") {
        const QDate last = QDate::fromString(kDay, Qt::ISODate);
        for (int i = 27; i >= 0; --i) {
            const QDate day = last.addDays(-i);
            const bool quiet = i % 7 == 2;
            const int minutes = quiet ? 0 : 40 + (i * 37) % 90;
            rows.append(tableRow(day.toString(Qt::ISODate), QLocale::c().toString(day, "ddd d MMM"), {},
                                 day.toString(Qt::ISODate), day.toString(Qt::ISODate), minutes, quiet ? 0 : 1 + i % 3,
                                 quiet ? -1 : 1.5 + (i % 5) * 0.8, quiet || i % 2 ? -1 : 3.0 + (i % 3) * 0.7,
                                 -1, quiet ? -1 : i % 4));
        }
    } else {
        rows = {tableRow("2031-W16", "17–20 Apr 2031", {}, "2031-04-17", "2031-04-20", 210, 7, 3.4, 4.0, 18, 3),
                tableRow("2031-W17", "21–27 Apr 2031", {}, "2031-04-21", "2031-04-27", 455, 16, 2.1, 3.2, 22, 9),
                tableRow("2031-W18", "28 Apr – 4 May 2031", {}, "2031-04-28", "2031-05-04", 520, 19, 3.8, 4.5, 30, 6),
                tableRow("2031-W19", "5–11 May 2031", {}, "2031-05-05", "2031-05-11", 0, 0, -1, -1, -1, -1),
                tableRow("2031-W20", "12–14 May 2031", {}, "2031-05-12", "2031-05-14", 182, 3, 4.6, 5.0, 12, 1)};
    }
    return QJsonObject{
        {"range", QJsonObject{{"from", "2031-04-17"}, {"to", kDay}}},
        {"filter", QJsonObject{{"tag_id", QJsonValue::Null}, {"project_id", QJsonValue::Null}, {"item_id", QJsonValue::Null}}},
        {"group", group},
        {"columns", QJsonArray{tableColumn("minutes", "Minutes", "min", "count", "sum"),
                               tableColumn("sessions", "Sessions", "sessions", "count", "sum"),
                               tableColumn("focus", "Focus", "", "scale", "avg", 5),
                               tableColumn("quality", "Quality", "", "scale", "avg", 5),
                               tableColumn("pages", "Pages read", "pages", "count", "sum"),
                               tableColumn("distractions", "Distractions", "times", "count", "sum"),
                               tableColumn("distractions_per_hour", "Distractions per hour", "per hour", "count", "avg", 0,
                                           "distractions")}},
        {"rows", rows},
        {"totals", QJsonObject{{"minutes", 1912}, {"sessions", 67},
                               {"values", QJsonObject{{"focus", 3.4}, {"quality", 4.3}, {"pages", 96.0},
                                                      {"distractions", 25.0}, {"distractions_per_hour", 0.8}}}}}};
}

// sessions.list rows (SessionView + project + item), shaped like sessions_list.json.
inline QJsonObject listRow(const QString &id, const QString &day, const QString &itemId, const QString &itemTitle,
                           const QString &projectId, const QString &start, int minutes, const QJsonObject &values,
                           const QString &note, const QJsonArray &notes = {}) {
    QJsonObject row = session(id, itemId, projectId, "fajr", minutes, values, start, false, note, notes);
    row["day"] = day;
    row["stale"] = false;
    row["project"] = projectId.isEmpty() ? QJsonValue(QJsonValue::Null) : QJsonValue(projectRef(projectId));
    row["item"] = itemId.isEmpty() ? QJsonValue(QJsonValue::Null)
                                   : QJsonValue(QJsonObject{{"id", itemId}, {"title", itemTitle}});
    return row;
}

inline QJsonObject sessionsList(bool filtered = false, bool more = false) {
    QJsonArray rows{
        listRow("l-1", kDay, "deep", "Deep work: grammar", "study", "2031-05-14T07:30:00Z", 40,
                QJsonObject{{"focus", 4.0}, {"distractions", 1.0}}, "Verb forms clicked today",
                QJsonArray{QJsonObject{{"id", "e-1"}, {"at", "2031-05-14T07:41:00Z"}, {"text", "Phone buzzed, put it away"}}}),
        listRow("l-2", kDay, "quran", "Read Quran", "quran", "2031-05-14T00:30:00Z", 25,
                QJsonObject{{"focus", 5.0}, {"quality", 5.0}, {"pages", 6.0}}, "Calm morning, slow recitation",
                QJsonArray{QJsonObject{{"id", "e-2"}, {"at", "2031-05-14T00:52:00Z"}, {"text", "Stopped at page 12"}}}),
        listRow("l-3", "2031-05-13", "gym", "Gym session", "fitness", "2031-05-13T12:10:00Z", 45,
                QJsonObject{{"focus", 2.0}, {"reps", 120.0}}, "Tired after work, short sets"),
        listRow("l-4", "2031-05-12", "", "", "", {}, 15, QJsonObject{{"focus", 1.0}}, "Evening walk, mind elsewhere"),
        listRow("l-5", "2031-05-11", "deep", "Deep work: grammar", "study", "2031-05-11T08:00:00Z", 60,
                QJsonObject{{"focus", 4.0}, {"quality", 3.0}, {"distractions", 2.0}},
                "Long session; the dative chapter needs a second pass")};
    if (filtered) rows = QJsonArray{rows.at(0), rows.at(1), rows.at(4)};
    return QJsonObject{{"rows", rows}, {"next", more ? QJsonValue("2031-05-11|2031-05-11T09:00:00Z|l-5")
                                                     : QJsonValue(QJsonValue::Null)}};
}

} // namespace TrackingFixture
