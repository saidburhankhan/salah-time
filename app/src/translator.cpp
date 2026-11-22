// Translation watching follows Omacut (MIT, David Heinemeier Hansson).
#include "translator.h"
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QGuiApplication>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLocale>
#include <QRegularExpression>
Translator::Translator(const QString&file,const QString&dir,QObject*p):QTranslator(p),m_file(file.isEmpty()?QDir::homePath()+"/.config/omarchy/language":file),m_dir(dir){m_debounce.setSingleShot(true);m_debounce.setInterval(80);connect(&m_watcher,&QFileSystemWatcher::directoryChanged,this,[this]{m_debounce.start();});connect(&m_watcher,&QFileSystemWatcher::fileChanged,this,[this]{m_debounce.start();});connect(&m_debounce,&QTimer::timeout,this,&Translator::reload);reload();}
QString Translator::readLanguage(const QString&p){QFile f(p);if(!f.open(QIODevice::ReadOnly))return"en";auto c=QString::fromUtf8(f.readLine()).trimmed();return QRegularExpression("^[a-z]{2,3}(-[A-Za-z]{2})?$").match(c).hasMatch()?c:"en";}
QString Translator::pluralCategory(const QString&l,long long c){long long n=qAbs(c),n10=n%10,n100=n%100;if(l=="ar"){if(n==0)return"zero";if(n==1)return"one";if(n==2)return"two";return n100>=3&&n100<=10?"few":n100>=11?"many":"other";}if(l=="ru"||l=="uk")return n10==1&&n100!=11?"one":n10>=2&&n10<=4&&!(n100>=12&&n100<=14)?"few":"many";if(l=="ja"||l.startsWith("zh"))return"other";return n==1?"one":"other";}
QString Translator::translate(const char*,const char*s,const char*,int n)const{auto v=m_strings.value(QString::fromUtf8(s));if(v.isString())return v.toString();if(v.isObject()){auto o=v.toObject();auto x=o.value(n<0?"other":pluralCategory(m_language,n)).toString();return x.isEmpty()?o.value("other").toString():x;}return{};}
void Translator::reload(){auto ps=m_watcher.files()+m_watcher.directories();if(!ps.isEmpty())m_watcher.removePaths(ps);auto d=QFileInfo(m_file).absolutePath();if(QFileInfo::exists(d))m_watcher.addPath(d);if(QFileInfo::exists(m_file))m_watcher.addPath(m_file);auto l=readLanguage(m_file);if(l==m_language)return;QHash<QString,QJsonValue>s;QStringList codes{"en",l.section('-',0,0),l};codes.removeDuplicates();for(auto&c:codes){QFile f(m_dir+"/"+c+".json");if(f.open(QIODevice::ReadOnly)){auto o=QJsonDocument::fromJson(f.readAll()).object();for(auto i=o.begin();i!=o.end();++i)if(i.value().isObject()||(i.value().isString()&&!i.value().toString().isEmpty()))s[i.key()]=i.value();}}m_language=l;m_strings=s;QLocale locale(l);QLocale::setDefault(locale);QGuiApplication::setLayoutDirection(locale.textDirection());emit changed();}
