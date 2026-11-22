#pragma once

#include <QAbstractListModel>
#include <QByteArray>
#include <QHash>
#include <QStringList>
#include <QVariantList>
#include <QVector>

// Base for the app's typed list models. Rows are keyed by a stable id so a
// refresh can be diffed into insert/remove/move/dataChanged signals instead of
// a model reset. Values are stored in role order: value column N belongs to the
// Nth role returned by roleNames(), whose role ids start at firstRole().
class TypedListModel : public QAbstractListModel {
    Q_OBJECT
    Q_PROPERTY(int count READ count NOTIFY countChanged)
public:
    explicit TypedListModel(QObject *parent = nullptr);

    int rowCount(const QModelIndex &parent = QModelIndex()) const override;
    QVariant data(const QModelIndex &index, int role) const override;
    QHash<int, QByteArray> roleNames() const override { return m_roles; }

    int count() const { return rowCount(); }
    int rowForKey(const QString &key) const;
    Q_INVOKABLE int indexOfKey(const QString &key) const { return rowForKey(key); }
    Q_INVOKABLE void moveRow(int from, int to);

protected:
    void setRoleNames(const QHash<int, QByteArray> &roles);
    void setRows(const QStringList &keys, const QVector<QVariantList> &values);
    const QStringList &keys() const { return m_keys; }
    const QVector<QVariantList> &values() const { return m_values; }
    QVariant valueAt(int row, int role) const;
    int firstRole() const { return m_firstRole; }

signals:
    void countChanged();

private:
    int roleColumn(int role) const { return role < m_firstRole ? -1 : role - m_firstRole; }
    QList<int> changedRoles(const QVariantList &before, const QVariantList &after) const;

    QHash<int, QByteArray> m_roles;
    int m_firstRole = Qt::UserRole + 1;
    QStringList m_keys;
    QVector<QVariantList> m_values;
};
