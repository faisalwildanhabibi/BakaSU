package org.bakasu.bakasu.ui.screen.hma

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.twotone.Add
import androidx.compose.material.icons.twotone.Apps
import androidx.compose.material.icons.twotone.Delete
import androidx.compose.material.icons.twotone.FolderOff
import androidx.compose.material.icons.twotone.Search
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import org.bakasu.bakasu.data.shell.KsuCliRepository
import org.bakasu.bakasu.domain.model.InstalledPackageInfo
import org.bakasu.bakasu.ui.component.PackageIcon
import org.bakasu.bakasu.ui.component.settings.SegmentedColumn
import org.bakasu.bakasu.ui.component.settings.SettingsBaseWidget
import org.bakasu.bakasu.ui.component.settings.SettingsSwitchWidget

@Composable
fun HmaPathsTab(
    pathsConfig: KsuCliRepository.HmaPathsConfig,
    installedPackages: List<InstalledPackageInfo>,
    contentPadding: PaddingValues,
    onToggleAutoHide: (Boolean) -> Unit,
    onSaveGlobalPaths: (List<String>) -> Unit,
    onSaveAppPaths: (String, List<String>) -> Unit,
) {
    var showGlobalEditor by remember { mutableStateOf(false) }
    var showAppPicker by remember { mutableStateOf(false) }
    var editingAppPkg by remember { mutableStateOf<String?>(null) }

    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = contentPadding,
    ) {
        item {
            SegmentedColumn(
                title = "TargetedHide Kernel Path Isolation",
                content = {
                    item {
                        SettingsSwitchWidget(
                            icon = Icons.TwoTone.FolderOff,
                            title = "Auto-Hide Hidden App Folders",
                            description = "Hide /data/app, /data/data, and /sdcard/Android/data of hidden apps via NoMount/SuSFS",
                            checked = pathsConfig.autoHideAppPaths,
                            onCheckedChange = onToggleAutoHide,
                        )
                    }
                    item {
                        SettingsBaseWidget(
                            icon = Icons.TwoTone.FolderOff,
                            title = "Global Hidden Paths",
                            description = "${pathsConfig.globalPaths.size} custom paths/!exceptions applied to all scoped apps",
                            onClick = { showGlobalEditor = true },
                        )
                    }
                    item {
                        SettingsBaseWidget(
                            icon = Icons.TwoTone.Add,
                            title = "Add Per-App Path Rule",
                            description = "Override global paths for a specific target app (ala <pkg>.txt)",
                            onClick = { showAppPicker = true },
                        )
                    }
                },
            )
        }

        item {
            val entries = pathsConfig.perAppPaths.entries.toList()
            SegmentedColumn(
                title = "Per-App Path Overrides (${entries.size})",
                content = {
                    if (entries.isEmpty()) {
                        item {
                            SettingsBaseWidget(
                                icon = Icons.TwoTone.Apps,
                                title = "No Per-App Overrides",
                                description = "All scoped apps currently inherit Global Hidden Paths",
                            )
                        }
                    } else {
                        entries.forEach { (pkg, list) ->
                            item {
                                SettingsBaseWidget(
                                    icon = Icons.TwoTone.Apps,
                                    title = pkg,
                                    description = "${list.size} custom rules",
                                    onClick = { editingAppPkg = pkg },
                                )
                            }
                        }
                    }
                },
            )
        }
    }

    if (showGlobalEditor) {
        PathListEditorDialog(
            title = "Global Hidden Paths",
            subtitle = "Applies to all scoped apps without per-app overrides. Prefix with ! for exceptions.",
            initialPaths = pathsConfig.globalPaths,
            onDismiss = { showGlobalEditor = false },
            onSave = {
                showGlobalEditor = false
                onSaveGlobalPaths(it)
            },
        )
    }

    if (showAppPicker) {
        SingleAppPickerDialog(
            installedPackages = installedPackages,
            onDismiss = { showAppPicker = false },
            onSelect = { pkg ->
                showAppPicker = false
                editingAppPkg = pkg.packageName
            },
        )
    }

    editingAppPkg?.let { pkg ->
        PathListEditorDialog(
            title = "Paths for $pkg",
            subtitle = "Leave empty to revert to Global Hidden Paths. Prefix with ! for exceptions.",
            initialPaths = pathsConfig.perAppPaths[pkg] ?: emptyList(),
            onDismiss = { editingAppPkg = null },
            onSave = {
                editingAppPkg = null
                onSaveAppPaths(pkg, it)
            },
        )
    }
}

@Composable
private fun PathListEditorDialog(
    title: String,
    subtitle: String,
    initialPaths: List<String>,
    onDismiss: () -> Unit,
    onSave: (List<String>) -> Unit,
) {
    var newPath by remember { mutableStateOf("") }
    val paths = remember { mutableStateListOf<String>().apply { addAll(initialPaths) } }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        text = {
            Column(modifier = Modifier.fillMaxWidth()) {
                Text(
                    text = subtitle,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(Modifier.height(8.dp))
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    OutlinedTextField(
                        value = newPath,
                        onValueChange = { newPath = it },
                        placeholder = { Text("/storage/emulated/0/Folder or !...") },
                        singleLine = true,
                        modifier = Modifier.weight(1f),
                    )
                    OutlinedButton(
                        enabled = newPath.trim().isNotEmpty(),
                        onClick = {
                            val trimmed = newPath.trim()
                            if (!paths.contains(trimmed)) {
                                paths.add(trimmed)
                            }
                            newPath = ""
                        },
                    ) {
                        Text("Add")
                    }
                }
                Spacer(Modifier.height(8.dp))
                LazyColumn(
                    modifier = Modifier
                        .fillMaxWidth()
                        .heightIn(max = 260.dp),
                ) {
                    items(paths, key = { it }) { item ->
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(vertical = 4.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text(
                                text = item,
                                style = MaterialTheme.typography.bodyMedium,
                                modifier = Modifier.weight(1f),
                            )
                            IconButton(onClick = { paths.remove(item) }) {
                                Icon(
                                    imageVector = Icons.TwoTone.Delete,
                                    contentDescription = "Remove path",
                                    tint = MaterialTheme.colorScheme.error,
                                )
                            }
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = { onSave(paths.toList()) }) {
                Text("Save")
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text("Cancel")
            }
        },
    )
}

@Composable
private fun SingleAppPickerDialog(
    installedPackages: List<InstalledPackageInfo>,
    onDismiss: () -> Unit,
    onSelect: (InstalledPackageInfo) -> Unit,
) {
    var query by remember { mutableStateOf("") }
    val filtered = remember(installedPackages, query) {
        val q = query.trim().lowercase()
        installedPackages.filter { pkg ->
            !pkg.isSystem &&
                (
                    q.isEmpty() ||
                        pkg.appLabel.lowercase().contains(q) ||
                        pkg.packageName.lowercase().contains(q)
                    )
        }.sortedBy { it.appLabel.lowercase() }
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Select Target App") },
        text = {
            Column(modifier = Modifier.fillMaxWidth()) {
                OutlinedTextField(
                    value = query,
                    onValueChange = { query = it },
                    placeholder = { Text("Search apps...") },
                    leadingIcon = { Icon(Icons.TwoTone.Search, contentDescription = null) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                Spacer(Modifier.height(8.dp))
                LazyColumn(
                    modifier = Modifier
                        .fillMaxWidth()
                        .heightIn(max = 320.dp),
                ) {
                    items(filtered, key = { it.packageName }) { pkg ->
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable { onSelect(pkg) }
                                .padding(vertical = 8.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            PackageIcon(
                                packageName = pkg.packageName,
                                contentDescription = pkg.appLabel,
                                modifier = Modifier.size(32.dp),
                            )
                            Spacer(Modifier.width(10.dp))
                            Column(modifier = Modifier.weight(1f)) {
                                Text(
                                    text = pkg.appLabel.ifEmpty { pkg.packageName },
                                    style = MaterialTheme.typography.bodyMedium,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                                Text(
                                    text = pkg.packageName,
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                        }
                    }
                }
            }
        },
        confirmButton = {},
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text("Cancel")
            }
        },
    )
}
