#include "typedlistmodel.h"

#include <QSet>
#include <algorithm>

TypedListModel::TypedListModel(QObject *parent) : QAbstractListModel(parent) {}

int TypedListModel::rowCount(const QModelIndex &parent) const {
    return parent.isValid() ? 0 : m_keys.size();
}

QVariant TypedListModel::data(const QModelIndex &index, int role) const {
    if (!index.isValid() || index.row() < 0 || index.row() >= m_keys.size()) return {};
    const int column = roleColumn(role);
    if (column < 0) return {};
    const QVariantList &row = m_values.at(index.row());
    return column < row.size() ? row.at(column) : QVariant{};
}

int TypedListModel::rowForKey(const QString &key) const {
    for (int i = 0; i < m_keys.size(); ++i)
        if (m_keys.at(i) == key) return i;
    return -1;
}

void TypedListModel::setRoleNames(const QHash<int, QByteArray> &roles) {
    m_roles = roles;
    m_firstRole = roles.isEmpty() ? Qt::UserRole + 1 : roles.constBegin().key();
    for (auto it = roles.constBegin(); it != roles.constEnd(); ++it)
        m_firstRole = std::min(m_firstRole, it.key());
}

QList<int> TypedListModel::changedRoles(const QVariantList &before, const QVariantList &after) const {
    QList<int> changed;
    const int columns = std::max(before.size(), after.size());
    for (int column = 0; column < columns; ++column) {
        const QVariant left = column < before.size() ? before.at(column) : QVariant{};
        const QVariant right = column < after.size() ? after.at(column) : QVariant{};
        if (left != right) changed.append(m_firstRole + column);
    }
    return changed;
}

void TypedListModel::setRows(const QStringList &keys, const QVector<QVariantList> &values) {
    Q_ASSERT(keys.size() == values.size());
    const int previousCount = m_keys.size();

    // Remove rows whose key disappeared, walking backwards so indexes stay
    // valid.
    QSet<QString> wanted;
    wanted.reserve(keys.size());
    for (const QString &key : keys) wanted.insert(key);
    for (int row = m_keys.size() - 1; row >= 0; --row) {
        if (wanted.contains(m_keys.at(row))) continue;
        beginRemoveRows(QModelIndex(), row, row);
        m_keys.removeAt(row);
        m_values.removeAt(row);
        endRemoveRows();
    }

    // Align surviving rows and insert new ones. Every reorder is a move signal,
    // never a reset, so delegates and scroll state survive.
    for (int row = 0; row < keys.size(); ++row) {
        if (row < m_keys.size() && m_keys.at(row) == keys.at(row)) continue;
        int found = -1;
        for (int probe = row + 1; probe < m_keys.size(); ++probe) {
            if (m_keys.at(probe) == keys.at(row)) { found = probe; break; }
        }
        if (found >= 0) {
            beginMoveRows(QModelIndex(), found, found, QModelIndex(), row);
            m_keys.move(found, row);
            m_values.move(found, row);
            endMoveRows();
        } else {
            beginInsertRows(QModelIndex(), row, row);
            m_keys.insert(row, keys.at(row));
            m_values.insert(row, values.at(row));
            endInsertRows();
        }
    }
    while (m_keys.size() > keys.size()) {
        const int row = m_keys.size() - 1;
        beginRemoveRows(QModelIndex(), row, row);
        m_keys.removeAt(row);
        m_values.removeAt(row);
        endRemoveRows();
    }

    // Update surviving rows in place and report only the roles that changed on
    // each affected row.
    for (int row = 0; row < m_keys.size(); ++row) {
        const QList<int> roles = changedRoles(m_values.at(row), values.at(row));
        if (roles.isEmpty()) {
            m_values[row] = values.at(row);
            continue;
        }
        m_values[row] = values.at(row);
        emit dataChanged(index(row, 0), index(row, 0), roles);
    }
    if (previousCount != m_keys.size()) emit countChanged();
}

QVariant TypedListModel::valueAt(int row, int role) const {
    if (row < 0 || row >= m_values.size()) return {};
    const int column = roleColumn(role);
    if (column < 0 || column >= m_values.at(row).size()) return {};
    return m_values.at(row).at(column);
}

void TypedListModel::moveRow(int from, int to) {
    if (from == to || from < 0 || to < 0 || from >= m_keys.size() || to >= m_keys.size()) return;
    const int destination = from < to ? to + 1 : to;
    if (!beginMoveRows(QModelIndex(), from, from, QModelIndex(), destination)) return;
    m_keys.move(from, to);
    m_values.move(from, to);
    endMoveRows();
}
