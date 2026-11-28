package org.bakasu.bakasu.ui.screen.hma

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.twotone.Delete
import androidx.compose.material.icons.twotone.Search
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Checkbox
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
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

@Composable
fun AppMultiSelectDialog(
    title: String,
    installedPackages: List<InstalledPackageInfo>,
    initialSelected: List<String>,
    onDismiss: () -> Unit,
    onConfirm: (List<String>) -> Unit,
) {
    var query by remember { mutableStateOf("") }
    var showSystem by remember { mutableStateOf(false) }
    val selected = remember { mutableStateListOf<String>().apply { addAll(initialSelected) } }

    val filtered = remember(installedPackages, query, showSystem, selected.size) {
        val q = query.trim().lowercase()
        installedPackages.filter { pkg ->
            (showSystem || !pkg.isSystem || selected.contains(pkg.packageName)) &&
                (
                    q.isEmpty() ||
                        pkg.appLabel.lowercase().contains(q) ||
                        pkg.packageName.lowercase().contains(q)
                    )
        }.sortedWith(
            compareByDescending<InstalledPackageInfo> { selected.contains(it.packageName) }
                .thenBy { it.appLabel.lowercase() },
        )
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
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
                Spacer(Modifier.height(6.dp))
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    FilterChip(
                        selected = showSystem,
                        onClick = { showSystem = !showSystem },
                        label = { Text("System Apps") },
                    )
                    Text(
                        text = "${selected.size} selected",
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.primary,
                    )
                }
                Spacer(Modifier.height(6.dp))
                LazyColumn(
                    modifier = Modifier
                        .fillMaxWidth()
                        .heightIn(max = 340.dp),
                ) {
                    items(filtered, key = { it.packageName }) { pkg ->
                        val isChecked = selected.contains(pkg.packageName)
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable {
                                    if (isChecked) {
                                        selected.remove(pkg.packageName)
                                    } else {
                                        selected.add(pkg.packageName)
                                    }
                                }
                                .padding(vertical = 6.dp),
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
                            Checkbox(
                                checked = isChecked,
                                onCheckedChange = { checked ->
                                    if (checked) {
                                        if (!selected.contains(pkg.packageName)) selected.add(pkg.packageName)
                                    } else {
                                        selected.remove(pkg.packageName)
                                    }
                                },
                            )
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = { onConfirm(selected.toList()) }) {
                Text("Apply")
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
fun HmaTemplateEditorDialog(
    existingTemplate: KsuCliRepository.HmaTemplate?,
    installedPackages: List<InstalledPackageInfo>,
    onDismiss: () -> Unit,
    onSave: (name: String, isWhitelist: Boolean, apps: List<String>) -> Unit,
    onDelete: ((name: String) -> Unit)? = null,
) {
    var name by remember { mutableStateOf(existingTemplate?.name ?: "") }
    var isWhitelist by remember { mutableStateOf(existingTemplate?.isWhitelist ?: false) }
    var appList by remember { mutableStateOf(existingTemplate?.appList ?: emptyList()) }
    var showAppPicker by remember { mutableStateOf(false) }

    val isNew = existingTemplate == null
    val isPreset = existingTemplate?.name in setOf(
        "root_apps",
        "detector_apps",
        "sus_apps",
        "custom_rom",
        "shizuku_dhizuku",
        "xposed",
        "accessibility",
    )
    val installedSet = remember(installedPackages) { installedPackages.map { it.packageName }.toSet() }
    val installedCount = appList.count { it in installedSet }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    when {
                        isNew -> "New Template"
                        isPreset -> "Edit Preset"
                        else -> "Edit Template"
                    },
                )
                if (!isNew && !isPreset && onDelete != null) {
                    IconButton(onClick = { onDelete(existingTemplate.name) }) {
                        Icon(
                            imageVector = Icons.TwoTone.Delete,
                            contentDescription = "Delete template",
                            tint = MaterialTheme.colorScheme.error,
                        )
                    }
                }
            }
        },
        text = {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                OutlinedTextField(
                    value = name,
                    onValueChange = { if (isNew) name = it },
                    label = { Text("Template Name") },
                    enabled = isNew,
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                if (isPreset) {
                    Text(
                        text = "Built-in hybrid preset: automatically synchronized with root-granted and manager packages.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.primary,
                    )
                }
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    FilterChip(
                        selected = !isWhitelist,
                        onClick = { isWhitelist = false },
                        label = { Text("Blacklist") },
                        modifier = Modifier.weight(1f),
                    )
                    FilterChip(
                        selected = isWhitelist,
                        onClick = { isWhitelist = true },
                        label = { Text("Whitelist") },
                        modifier = Modifier.weight(1f),
                    )
                }
                OutlinedButton(
                    onClick = { showAppPicker = true },
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text("Select Apps ($installedCount installed / ${appList.size} total)")
                }
            }
        },
        confirmButton = {
            TextButton(
                enabled = name.trim().isNotEmpty(),
                onClick = { onSave(name.trim(), isWhitelist, appList) },
            ) {
                Text("Save")
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text("Cancel")
            }
        },
    )

    if (showAppPicker) {
        AppMultiSelectDialog(
            title = if (isWhitelist) "Allowed Apps (Whitelist)" else "Hidden Apps (Blacklist)",
            installedPackages = installedPackages,
            initialSelected = appList,
            onDismiss = { showAppPicker = false },
            onConfirm = {
                appList = it
                showAppPicker = false
            },
        )
    }
}

@Composable
fun HmaScopeEditorDialog(
    pkg: InstalledPackageInfo,
    initialScope: KsuCliRepository.HmaAppScope?,
    templates: Map<String, KsuCliRepository.HmaTemplate>,
    settingsTemplates: Map<String, List<String>> = emptyMap(),
    installedPackages: List<InstalledPackageInfo>,
    onDismiss: () -> Unit,
    onSave: (enabled: Boolean, scope: KsuCliRepository.HmaAppScope) -> Unit,
) {
    var enabled by remember { mutableStateOf(initialScope != null) }
    var useWhitelist by remember { mutableStateOf(initialScope?.useWhitelist ?: false) }
    var excludeSystemApps by remember { mutableStateOf(initialScope?.excludeSystemApps ?: true) }
    var hideInstallationSource by remember { mutableStateOf(initialScope?.hideInstallationSource ?: false) }
    var hideSystemInstallationSource by remember { mutableStateOf(initialScope?.hideSystemInstallationSource ?: false) }
    var excludeTargetInstallationSource by remember { mutableStateOf(initialScope?.excludeTargetInstallationSource ?: false) }
    var invertActivityLaunchProtection by remember { mutableStateOf(initialScope?.invertActivityLaunchProtection ?: false) }

    val officialAppPresets = remember {
        listOf(
            "root_apps" to "Root Apps (Magisk, KernelSU, APatch, AdGuard, Termux)",
            "detector_apps" to "Root & Environment Detectors (Momo, Ruru, Native Detector)",
            "sus_apps" to "Suspicious & Forensic Apps",
            "custom_rom" to "Custom ROM Identifier Packages",
            "shizuku_dhizuku" to "Shizuku & Dhizuku Services",
            "xposed" to "Xposed & LSPosed Managers",
            "accessibility_apps" to "Accessibility Service Applications",
        )
    }

    val officialSettingsPresets = remember {
        listOf(
            "accessibility" to "Aksesibilitas (Hide accessibility status & active services)",
            "input_method" to "Alat Input (Spoof default input method & TTS synth)",
            "dev_options" to "Opsi Pengembang (Hide ADB, USB debugging, dev options)",
        )
    }

    val appliedPresets = remember {
        mutableStateListOf<String>().apply {
            addAll(initialScope?.applyPresets ?: emptyList())
        }
    }
    val appliedSettingsPresets = remember {
        mutableStateListOf<String>().apply {
            addAll(initialScope?.applySettingsPresets ?: emptyList())
        }
    }
    val appliedSettingTemplates = remember {
        mutableStateListOf<String>().apply {
            addAll(initialScope?.applySettingTemplates ?: emptyList())
        }
    }
    val appliedTemplates = remember {
        mutableStateListOf<String>().apply {
            addAll(initialScope?.applyTemplates ?: emptyList())
        }
    }
    var extraAppList by remember { mutableStateOf(initialScope?.extraAppList ?: emptyList()) }
    var showExtraPicker by remember { mutableStateOf(false) }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = {
            Column {
                Text(
                    text = pkg.appLabel.ifEmpty { pkg.packageName },
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
        },
        text = {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                DialogSwitchRow(
                    title = "Enable Hide",
                    subtitle = "Apply HMA & UID Seccomp isolation to this app",
                    checked = enabled,
                    onCheckedChange = { enabled = it },
                )

                if (enabled) {
                    HorizontalDivider()
                    Text(
                        text = "Work Mode",
                        style = MaterialTheme.typography.labelLarge,
                        color = MaterialTheme.colorScheme.primary,
                    )
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        FilterChip(
                            selected = !useWhitelist,
                            onClick = { useWhitelist = false },
                            label = { Text("Blacklist") },
                            modifier = Modifier.weight(1f),
                        )
                        FilterChip(
                            selected = useWhitelist,
                            onClick = { useWhitelist = true },
                            label = { Text("Whitelist") },
                            modifier = Modifier.weight(1f),
                        )
                    }

                    if (useWhitelist) {
                        DialogSwitchRow(
                            title = "Exclude System Apps",
                            subtitle = "Keep system apps visible in whitelist mode",
                            checked = excludeSystemApps,
                            onCheckedChange = { excludeSystemApps = it },
                        )
                    }

                    HorizontalDivider()
                    Text(
                        text = "App Presets (Built-in)",
                        style = MaterialTheme.typography.labelLarge,
                        color = MaterialTheme.colorScheme.primary,
                    )
                    officialAppPresets.forEach { (presetKey, presetLabel) ->
                        val isSelected = appliedPresets.contains(presetKey)
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable {
                                    if (isSelected) {
                                        appliedPresets.remove(presetKey)
                                    } else {
                                        appliedPresets.add(presetKey)
                                    }
                                }
                                .padding(vertical = 2.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Checkbox(
                                checked = isSelected,
                                onCheckedChange = { checked ->
                                    if (checked) {
                                        if (!appliedPresets.contains(presetKey)) appliedPresets.add(presetKey)
                                    } else {
                                        appliedPresets.remove(presetKey)
                                    }
                                },
                            )
                            Column(modifier = Modifier.weight(1f)) {
                                Text(
                                    text = presetKey,
                                    style = MaterialTheme.typography.bodyMedium,
                                )
                                Text(
                                    text = presetLabel,
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        }
                    }

                    HorizontalDivider()
                    Text(
                        text = "Settings Presets (Aksesibilitas, Input, Dev)",
                        style = MaterialTheme.typography.labelLarge,
                        color = MaterialTheme.colorScheme.primary,
                    )
                    officialSettingsPresets.forEach { (spKey, spLabel) ->
                        val isSelected = appliedSettingsPresets.contains(spKey)
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable {
                                    if (isSelected) {
                                        appliedSettingsPresets.remove(spKey)
                                    } else {
                                        appliedSettingsPresets.add(spKey)
                                    }
                                }
                                .padding(vertical = 2.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Checkbox(
                                checked = isSelected,
                                onCheckedChange = { checked ->
                                    if (checked) {
                                        if (!appliedSettingsPresets.contains(spKey)) appliedSettingsPresets.add(spKey)
                                    } else {
                                        appliedSettingsPresets.remove(spKey)
                                    }
                                },
                            )
                            Column(modifier = Modifier.weight(1f)) {
                                Text(
                                    text = spKey,
                                    style = MaterialTheme.typography.bodyMedium,
                                )
                                Text(
                                    text = spLabel,
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        }
                    }

                    if (settingsTemplates.isNotEmpty()) {
                        HorizontalDivider()
                        Text(
                            text = "Custom Setting Templates",
                            style = MaterialTheme.typography.labelLarge,
                            color = MaterialTheme.colorScheme.primary,
                        )
                        settingsTemplates.forEach { (stName, stItems) ->
                            val isSelected = appliedSettingTemplates.contains(stName)
                            Row(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .clickable {
                                        if (isSelected) {
                                            appliedSettingTemplates.remove(stName)
                                        } else {
                                            appliedSettingTemplates.add(stName)
                                        }
                                    }
                                    .padding(vertical = 2.dp),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                Checkbox(
                                    checked = isSelected,
                                    onCheckedChange = { checked ->
                                        if (checked) {
                                            if (!appliedSettingTemplates.contains(stName)) appliedSettingTemplates.add(stName)
                                        } else {
                                            appliedSettingTemplates.remove(stName)
                                        }
                                    },
                                )
                                Text(
                                    text = "$stName (${stItems.size} rules)",
                                    style = MaterialTheme.typography.bodyMedium,
                                )
                            }
                        }
                    }

                    HorizontalDivider()
                    Text(
                        text = "User Templates",
                        style = MaterialTheme.typography.labelLarge,
                        color = MaterialTheme.colorScheme.primary,
                    )
                    val userTemplates = templates.values.filter {
                        it.isWhitelist == useWhitelist && it.name !in setOf(
                            "root_apps",
                            "detector_apps",
                            "sus_apps",
                            "custom_rom",
                            "shizuku_dhizuku",
                            "xposed",
                            "accessibility_apps",
                            "accessibility",
                        )
                    }
                    if (userTemplates.isEmpty()) {
                        Text(
                            text = "No user ${if (useWhitelist) "whitelist" else "blacklist"} templates",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    } else {
                        userTemplates.forEach { tpl ->
                            val isSelected = appliedTemplates.contains(tpl.name)
                            Row(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .clickable {
                                        if (isSelected) {
                                            appliedTemplates.remove(tpl.name)
                                        } else {
                                            appliedTemplates.add(tpl.name)
                                        }
                                    }
                                    .padding(vertical = 2.dp),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                Checkbox(
                                    checked = isSelected,
                                    onCheckedChange = { checked ->
                                        if (checked) {
                                            if (!appliedTemplates.contains(tpl.name)) appliedTemplates.add(tpl.name)
                                        } else {
                                            appliedTemplates.remove(tpl.name)
                                        }
                                    },
                                )
                                Text(
                                    text = "${tpl.name} (${tpl.appList.size} apps)",
                                    style = MaterialTheme.typography.bodyMedium,
                                )
                            }
                        }
                    }

                    OutlinedButton(
                        onClick = { showExtraPicker = true },
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Text("Extra Apps (${extraAppList.size})")
                    }

                    HorizontalDivider()
                    Text(
                        text = "Advanced Flags",
                        style = MaterialTheme.typography.labelLarge,
                        color = MaterialTheme.colorScheme.primary,
                    )
                    DialogSwitchRow(
                        title = "Hide Installation Source",
                        subtitle = "Spoof installer package for user apps",
                        checked = hideInstallationSource,
                        onCheckedChange = { hideInstallationSource = it },
                    )
                    DialogSwitchRow(
                        title = "Hide System Installer",
                        subtitle = "Spoof installer package for system apps",
                        checked = hideSystemInstallationSource,
                        onCheckedChange = { hideSystemInstallationSource = it },
                    )
                    DialogSwitchRow(
                        title = "Exclude Target Installer",
                        subtitle = "Do not spoof this app's own installer",
                        checked = excludeTargetInstallationSource,
                        onCheckedChange = { excludeTargetInstallationSource = it },
                    )
                    DialogSwitchRow(
                        title = "Invert Activity Protection",
                        subtitle = "Flip global activity launch protection for this app",
                        checked = invertActivityLaunchProtection,
                        onCheckedChange = { invertActivityLaunchProtection = it },
                    )
                }
            }
        },
        confirmButton = {
            TextButton(
                onClick = {
                    onSave(
                        enabled,
                        KsuCliRepository.HmaAppScope(
                            packageName = pkg.packageName,
                            useWhitelist = useWhitelist,
                            excludeSystemApps = excludeSystemApps,
                            hideInstallationSource = hideInstallationSource,
                            hideSystemInstallationSource = hideSystemInstallationSource,
                            excludeTargetInstallationSource = excludeTargetInstallationSource,
                            invertActivityLaunchProtection = invertActivityLaunchProtection,
                            applyTemplates = appliedTemplates.filter { templates[it]?.isWhitelist == useWhitelist },
                            applyPresets = appliedPresets.toList(),
                            applySettingsPresets = appliedSettingsPresets.toList(),
                            applySettingTemplates = appliedSettingTemplates.toList(),
                            extraAppList = extraAppList,
                        ),
                    )
                },
            ) {
                Text("Save")
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text("Cancel")
            }
        },
    )

    if (showExtraPicker) {
        AppMultiSelectDialog(
            title = if (useWhitelist) "Extra Allowed Apps" else "Extra Hidden Apps",
            installedPackages = installedPackages,
            initialSelected = extraAppList,
            onDismiss = { showExtraPicker = false },
            onConfirm = {
                extraAppList = it
                showExtraPicker = false
            },
        )
    }
}

@Composable
private fun DialogSwitchRow(
    title: String,
    subtitle: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { onCheckedChange(!checked) }
            .padding(vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(text = title, style = MaterialTheme.typography.bodyMedium)
            Text(
                text = subtitle,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Spacer(Modifier.width(8.dp))
        Switch(checked = checked, onCheckedChange = onCheckedChange)
    }
}
