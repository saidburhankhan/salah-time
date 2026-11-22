#include "routinesmodel.h"

RoutinesModel::RoutinesModel(QObject *parent) : StatsItemsModel(parent) {}

void RoutinesModel::setStats(const Dto::Stats &stats) {
    std::vector<Dto::StatsItem> routines;
    for (const Dto::StatsItem &item : stats.items)
        if (item.repeat != QStringLiteral("none")) routines.push_back(item);
    setItems(routines, Sort::ServerOrder);
}
