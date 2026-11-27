package org.bakasu.bakasu.ui.screen.hma

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.twotone.Add
import androidx.compose.material.icons.twotone.ListAlt
import androidx.compose.material.icons.twotone.Refresh
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import org.bakasu.bakasu.data.shell.KsuCliRepository
import org.bakasu.bakasu.domain.model.InstalledPackageInfo
import org.bakasu.bakasu.ui.component.settings.SegmentedColumn
import org.bakasu.bakasu.ui.component.settings.SettingsBaseWidget

@Composable
fun HmaTemplatesTab(
    config: KsuCliRepository.HmaConfig,
    installedPackages: List<InstalledPackageInfo>,
    contentPadding: PaddingValues,
    onSaveTemplate: (name: String, isWhitelist: Boolean, apps: List<String>) -> Unit,
    onDeleteTemplate: (name: String) -> Unit,
    onReloadPresets: () -> Unit,
) {
    var showCreateDialog by remember { mutableStateOf(false) }
    var editingTemplate by remember { mutableStateOf<KsuCliRepository.HmaTemplate?>(null) }

    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = contentPadding,
    ) {
        item {
            SegmentedColumn(
                title = "Template Actions",
                content = {
                    item {
                        SettingsBaseWidget(
                            icon = Icons.TwoTone.Add,
                            title = "Create New Template",
                            description = "Add a reusable blacklist or whitelist app template",
                            onClick = { showCreateDialog = true },
                        )
                    }
                    item {
                        SettingsBaseWidget(
                            icon = Icons.TwoTone.Refresh,
                            title = "Reload Presets",
                            description = "Rescan installed packages and refresh official HMA presets",
                            onClick = { onReloadPresets() },
                        )
                    }
                },
            )
        }

        item {
            val officialPresetNames = remember {
                setOf(
                    "root_apps",
                    "detector_apps",
                    "sus_apps",
                    "custom_rom",
                    "shizuku_dhizuku",
                    "xposed",
                    "accessibility",
                )
            }
            val installedSet = remember(installedPackages) {
                installedPackages.map { it.packageName }.toSet()
            }
            val presets = config.templates.values.filter { it.name in officialPresetNames }
            val templates = config.templates.values.filter { it.name !in officialPresetNames }

            if (presets.isNotEmpty()) {
                SegmentedColumn(
                    title = "Official Presets (${presets.size})",
                    content = {
                        presets.forEach { preset ->
                            item {
                                val mode = if (preset.isWhitelist) "Whitelist" else "Blacklist"
                                val installedCount = preset.appList.count { it in installedSet }
                                SettingsBaseWidget(
                                    icon = Icons.TwoTone.ListAlt,
                                    title = "Official Preset (${preset.name})",
                                    description = "$mode • $installedCount installed / ${preset.appList.size} total • Auto-Synced",
                                    onClick = { editingTemplate = preset },
                                )
                            }
                        }
                    },
                )
            }

            SegmentedColumn(
                title = "User Templates (${templates.size})",
                content = {
                    if (templates.isEmpty()) {
                        item {
                            SettingsBaseWidget(
                                icon = Icons.TwoTone.ListAlt,
                                title = "No User Templates",
                                description = "Tap Create New Template above to add one",
                            )
                        }
                    } else {
                        templates.forEach { tpl ->
                            item {
                                val mode = if (tpl.isWhitelist) "Whitelist" else "Blacklist"
                                val installedCount = tpl.appList.count { it in installedSet }
                                SettingsBaseWidget(
                                    icon = Icons.TwoTone.ListAlt,
                                    title = tpl.name,
                                    description = "$mode • $installedCount installed / ${tpl.appList.size} total",
                                    onClick = { editingTemplate = tpl },
                                )
                            }
                        }
                    }
                },
            )
        }
    }

    if (showCreateDialog) {
        HmaTemplateEditorDialog(
            existingTemplate = null,
            installedPackages = installedPackages,
            onDismiss = { showCreateDialog = false },
            onSave = { name, isWhitelist, apps ->
                showCreateDialog = false
                onSaveTemplate(name, isWhitelist, apps)
            },
        )
    }

    editingTemplate?.let { tpl ->
        HmaTemplateEditorDialog(
            existingTemplate = tpl,
            installedPackages = installedPackages,
            onDismiss = { editingTemplate = null },
            onSave = { name, isWhitelist, apps ->
                editingTemplate = null
                onSaveTemplate(name, isWhitelist, apps)
            },
            onDelete = { name ->
                editingTemplate = null
                onDeleteTemplate(name)
            },
        )
    }
}
