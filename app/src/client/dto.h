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
struct DayItem { QJsonObject json; QString id, title, block, repeat, measure, state; };
struct DayBlock { QJsonObject json; QString start, end; std::vector<DayItem> items; };
struct Prayer { QJsonObject json; QString id; };
struct Now { QJsonObject json; QString block; };
struct Sleep { QJsonObject json; QString time; };
struct DayPlan { QJsonObject json; QString day; std::vector<DayBlock> blocks; std::vector<Prayer> prayers; std::optional<Now> now; Sleep sleep; };
struct Today { QJsonObject json; QString day, block; std::vector<DayItem> items, current, upcoming; };
struct StatsSeries { QJsonObject json; QString day; };
struct StatsItem { QJsonObject json; QString id, title, block, repeat, measure; std::vector<StatsSeries> series; };
struct StatsDaily { QJsonObject json; QString day; };
struct StatsTotals { QJsonObject json; };
struct StatsPrayer { QJsonObject json; QString prayer; };
struct Stats { QJsonObject json; QString from, to; std::vector<StatsItem> items; std::vector<StatsDaily> daily; std::vector<StatsPrayer> prayers; StatsTotals totals; };
struct Item { QJsonObject json; QString id, title, block, repeat, measure; };
struct Entry { QJsonObject json; QString id, itemId, kind; };
struct PrayerMark { QJsonObject json; QString id, day, prayer; };
struct ParseDraft { QJsonObject json; QString title, repeat, measure; };
struct Config { QJsonObject json; QString location, method, sleepTime; };
struct Undo { QJsonObject json; int undone = 0; };
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
                           std::vector<Entry>, PrayerMark, ParseDraft, Config, Undo,
                           MutationResult>;
using AnyDecoded = Decoded<Value>;

template <typename T> Decoded<T> decode(const QJsonValue &value);
AnyDecoded decodeDayPlan(const QJsonValue &); AnyDecoded decodeToday(const QJsonValue &);
AnyDecoded decodeStats(const QJsonValue &); AnyDecoded decodeItem(const QJsonValue &);
AnyDecoded decodeEntry(const QJsonValue &); AnyDecoded decodeEntries(const QJsonValue &);
AnyDecoded decodePrayerMark(const QJsonValue &); AnyDecoded decodeParseDraft(const QJsonValue &);
AnyDecoded decodeConfig(const QJsonValue &); AnyDecoded decodeUndo(const QJsonValue &);
AnyDecoded decodeMutationResult(const QJsonValue &);
QJsonObject jsonObject(const Value &value);

} // namespace Dto
