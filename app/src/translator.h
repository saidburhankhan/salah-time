#pragma once
#include <QFileSystemWatcher>
#include <QHash>
#include <QJsonValue>
#include <QTimer>
#include <QTranslator>
class Translator:public QTranslator{
 Q_OBJECT Q_PROPERTY(QString language READ language NOTIFY changed)
public: explicit Translator(const QString &languageFile={},const QString &catalogDirectory=":/i18n",QObject *parent=nullptr);
 QString language()const{return m_language;} bool isEmpty()const override{return m_strings.isEmpty();}
 QString translate(const char *, const char *, const char * = nullptr, int = -1) const override;
 static QString readLanguage(const QString&); static QString pluralCategory(const QString&,long long);
signals:void changed();
private:void reload();QString m_file,m_dir,m_language;QHash<QString,QJsonValue>m_strings;QFileSystemWatcher m_watcher;QTimer m_debounce;
};
