// Command-line handling for `salah-app --page <name>`.
#pragma once
#include <QString>
#include <QStringList>

namespace launch {
inline const QStringList &pageNames() {
    static const QStringList names{"today", "backlog", "routines", "statistics", "insights", "goals", "review", "settings"};
    return names;
}

// The page requested by `--page <name>` or `--page=<name>`; "today" when absent
// or unknown, with `warning` set for an unknown or missing name.
inline QString startPage(const QStringList &arguments, QString *warning = nullptr) {
    for (int i = 1; i < arguments.size(); ++i) {
        QString value;
        if (arguments.at(i) == QLatin1String("--page")) {
            if (i + 1 >= arguments.size()) {
                if (warning) *warning = QStringLiteral("salah-app: --page needs a page name; opening Today");
                return QStringLiteral("today");
            }
            value = arguments.at(i + 1);
        } else if (arguments.at(i).startsWith(QLatin1String("--page="))) {
            value = arguments.at(i).mid(7);
        } else {
            continue;
        }
        value = value.trimmed().toLower();
        if (pageNames().contains(value)) return value;
        if (warning)
            *warning = QStringLiteral("salah-app: unknown page \"%1\" (use %2); opening Today")
                           .arg(value, pageNames().join(QLatin1String(", ")));
        return QStringLiteral("today");
    }
    return QStringLiteral("today");
}
} // namespace launch
