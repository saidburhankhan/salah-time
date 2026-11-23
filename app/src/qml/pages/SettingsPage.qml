import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"
import "../settings"

Item {
    id: settingsPage
    objectName: "settingsPage"
    property var win
    // general | projects | tags | metrics
    property string section: "general"
    readonly property var sections: [
        { id: "general", name: qsTr("General") },
        { id: "projects", name: qsTr("Projects") },
        { id: "tags", name: qsTr("Tags") },
        { id: "metrics", name: qsTr("Metrics") }
    ]

    onVisibleChanged: {
        if (visible)
            backend.loadSettings()
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 28
        spacing: 20

        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            Repeater {
                model: settingsPage.sections
                ChoiceChip {
                    required property var modelData
                    win: settingsPage.win
                    text: modelData.name
                    selected: settingsPage.section === modelData.id
                    onClicked: settingsPage.section = modelData.id
                }
            }
            Item {
                Layout.fillWidth: true
            }
        }

        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: ["general", "projects", "tags", "metrics"].indexOf(settingsPage.section)

            GeneralSettings {
                win: settingsPage.win
            }
            ProjectsSettings {
                objectName: "projectsSettings"
                win: settingsPage.win
            }
            TagsSettings {
                objectName: "tagsSettings"
                win: settingsPage.win
            }
            MetricsSettings {
                objectName: "metricsSettings"
                win: settingsPage.win
            }
        }
    }
}
