#pragma once
#include <QFileSystemWatcher>
#include <QObject>
#include <QProcess>
#include <QTimer>
class Theme : public QObject {
 Q_OBJECT
 Q_PROPERTY(QString accent READ accent NOTIFY changed)
 Q_PROPERTY(QString accentForeground READ accentForeground NOTIFY changed)
 Q_PROPERTY(int radius READ radius NOTIFY changed)
public:
 explicit Theme(const QString &directory = {}, QObject *parent = nullptr, const QString &hyprctl = {});
 ~Theme() override;
 QString accent() const { return m_accent; } QString accentForeground() const; int radius() const { return m_radius; }
 static QString readAccent(const QString &path);
signals: void changed();
private:
 void reload(); void refreshRounding();
 QString m_directory, m_accent = "#FFD60A", m_hyprctl; int m_radius = 0;
 QFileSystemWatcher m_watcher; QTimer m_debounce, m_poll, m_timeout; QProcess m_process;
};
