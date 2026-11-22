#pragma once

#include "statsitemsmodel.h"

// The routines view is the repeating subset of the 14-day stats projection.
// Keeping it as a model (rather than a QML filter over a map list) lets the
// route survive refetches by id.
class RoutinesModel : public StatsItemsModel {
    Q_OBJECT
public:
    explicit RoutinesModel(QObject *parent = nullptr);

    void setStats(const Dto::Stats &stats);
};
