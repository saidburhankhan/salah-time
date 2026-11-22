#pragma once

#include <QJsonObject>
#include <QJsonValue>
#include <QString>
#include <QStringList>
#include <optional>
#include <variant>
#include <vector>

namespace Dto {

struct DecodeError { QString path; QString message; QString formatted() const; };

template <typename T> struct Decoded {
    std::optional<T> value;
    std::optional<DecodeError> error;
    explicit operator bool() const { return value.has_value(); }
};

// DTOs intentionally keep their source object. Backend is the temporary adapter
// from these checked values to the QVariant shapes consumed by existing QML.
struct DayItem {
    QJsonObject json;
    QString id, title, block, repeat, measure, state;
    // Protocol v1 display projections. They are absent from the frozen legacy
    // CLI JSON, so the adapter also has to accept rows without them.
    QString schedule_text, detail_text, next_occurrence;
    bool carried = false;
    bool timer_running = false;
    QString timer_started_at;
    qint64 timer_minutes_including_running = 0;
};
struct DayBlock { QJsonObject json; QString start, end; std::vector<DayItem> items; };
struct Prayer { QJsonObject json; QString id; };
struct Now { QJsonObject json; QString block, next_at; };
struct Sleep { QJsonObject json; QString time; };
struct DayPlan { QJsonObject json; QString day; std::vector<DayBlock> blocks; std::vector<Prayer> prayers; std::optional<Now> now; Sleep sleep; };
struct Today { QJsonObject json; QString day, block; std::vector<DayItem> items, current, upcoming; };
struct StatsSeries { QJsonObject json; QString day; };
struct StatsItem { QJsonObject json; QString id, title, block, repeat, measure, schedule_text, next_occurrence; std::vector<StatsSeries> series; };
struct StatsDaily { QJsonObject json; QString day, tooltip; };
struct StatsTotals { QJsonObject json; };
struct StatsTick { QJsonObject json; qint64 minutes = 0; QString label; };
struct StatsPrayer { QJsonObject json; QString prayer; };
struct Stats { QJsonObject json; QString from, to; std::vector<StatsItem> items; std::vector<StatsDaily> daily; std::vector<StatsPrayer> prayers; StatsTotals totals; std::vector<StatsTick> time_ticks; };
struct Item { QJsonObject json; QString id, title, block, repeat, measure; };
struct Entry { QJsonObject json; QString id, itemId, kind; };
struct PrayerMark { QJsonObject json; QString id, day, prayer; };
struct ParseDraft { QJsonObject json; QString title, repeat; int plannedMinutes = 0; bool hasPlannedMinutes = false; };
// item.preview result for the entry form and quick-add toast.
struct Preview { QJsonObject json; QString next_occurrence, schedule_text; int occurrences_this_month = 0; };
struct Config { QJsonObject json; QString location, method, sleepTime, asr; };
struct Undo { QJsonObject json; int undone = 0; };
struct Json { QJsonValue value; QJsonObject json; };
// Canonical protocol v1 mutation result: the projection rows the UI renders.
struct MutationResult {
    QJsonObject json;
    qint64 revision = 0;
    QString opId;
    std::vector<DayItem> items;
    std::vector<Entry> entries;
    std::vector<PrayerMark> marks;
    QStringList deleted;
};

using Value = std::variant<std::monostate, DayPlan, Today, Stats, Item, Entry,
                           std::vector<Entry>, PrayerMark, ParseDraft, Preview, Config,
                           Undo, MutationResult, Json>;
using AnyDecoded = Decoded<Value>;

template <typename T> Decoded<T> decode(const QJsonValue &value);
AnyDecoded decodeDayPlan(const QJsonValue &); AnyDecoded decodeToday(const QJsonValue &);
AnyDecoded decodeStats(const QJsonValue &); AnyDecoded decodeItem(const QJsonValue &);
AnyDecoded decodeEntry(const QJsonValue &); AnyDecoded decodeEntries(const QJsonValue &);
AnyDecoded decodePrayerMark(const QJsonValue &); AnyDecoded decodeParseDraft(const QJsonValue &);
AnyDecoded decodeConfig(const QJsonValue &); AnyDecoded decodeUndo(const QJsonValue &);
AnyDecoded decodePreview(const QJsonValue &);
AnyDecoded decodeMutationResult(const QJsonValue &);
AnyDecoded decodeJson(const QJsonValue &);
QJsonObject jsonObject(const Value &value);

} // namespace Dto
