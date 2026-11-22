// Theme watching follows Omacut (MIT, David Heinemeier Hansson).
#include "theme.h"
#include <QColor>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonDocument>
#include <QJsonObject>
#include <QRegularExpression>
#include <QStandardPaths>
#include <cmath>
Theme::Theme(const QString &directory, QObject *parent, const QString &hyprctl) : QObject(parent),
 m_directory(directory.isEmpty() ? QDir::homePath()+"/.local/state/omarchy/current" : directory) {
 m_debounce.setSingleShot(true); m_debounce.setInterval(80);
 connect(&m_watcher,&QFileSystemWatcher::directoryChanged,this,[this]{m_debounce.start();});
 connect(&m_watcher,&QFileSystemWatcher::fileChanged,this,[this]{m_debounce.start();});
 connect(&m_debounce,&QTimer::timeout,this,&Theme::reload);
 m_hyprctl=!hyprctl.isEmpty()?hyprctl:directory.isEmpty()&&!qEnvironmentVariableIsEmpty("HYPRLAND_INSTANCE_SIGNATURE")?QStandardPaths::findExecutable("hyprctl"):QString();
 m_timeout.setSingleShot(true); m_timeout.setInterval(500); connect(&m_timeout,&QTimer::timeout,&m_process,&QProcess::kill);
 connect(&m_process,&QProcess::finished,this,[this](int code,QProcess::ExitStatus status){m_timeout.stop(); auto v=QJsonDocument::fromJson(m_process.readAllStandardOutput()).object().value("int"); if(code==0&&status==QProcess::NormalExit&&v.isDouble()&&v.toInt()>=0&&v.toInt()!=m_radius){m_radius=v.toInt();emit changed();}});
 m_poll.setInterval(1000); connect(&m_poll,&QTimer::timeout,this,&Theme::refreshRounding); if(!m_hyprctl.isEmpty())m_poll.start(); reload();
}
Theme::~Theme(){if(m_process.state()!=QProcess::NotRunning){m_process.kill();m_process.waitForFinished(1000);}}
void Theme::refreshRounding(){if(m_hyprctl.isEmpty()||m_process.state()!=QProcess::NotRunning)return;m_process.start(m_hyprctl,{"-j","getoption","decoration:rounding"});m_timeout.start();}
QString Theme::readAccent(const QString &path){QFile f(path);if(f.open(QIODevice::ReadOnly)){QRegularExpression r(R"re(^\s*accent\s*=\s*["'](#[0-9a-fA-F]{6})["'])re",QRegularExpression::MultilineOption);auto m=r.match(QString::fromUtf8(f.readAll()));if(m.hasMatch())return m.captured(1);}return "#FFD60A";}
QString Theme::accentForeground()const{QColor c(m_accent);auto l=[](double v){return v<=.04045?v/12.92:std::pow((v+.055)/1.055,2.4);};return .2126*l(c.redF())+.7152*l(c.greenF())+.0722*l(c.blueF())>.179?"black":"white";}
void Theme::reload(){refreshRounding();auto paths=m_watcher.files()+m_watcher.directories();if(!paths.isEmpty())m_watcher.removePaths(paths);QString ancestor=m_directory;while(!QFileInfo::exists(ancestor)&&ancestor!="/")ancestor=QFileInfo(ancestor).absolutePath();QStringList c{ancestor,QFileInfo(ancestor).absolutePath(),m_directory,m_directory+"/theme",m_directory+"/theme/colors.toml"};c.removeDuplicates();for(const auto&p:c)if(QFileInfo::exists(p))m_watcher.addPath(p);auto a=readAccent(m_directory+"/theme/colors.toml");if(a!=m_accent){m_accent=a;emit changed();}}
