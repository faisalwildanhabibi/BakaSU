package org.bakasu.bakasu.ui.screen.hma

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.twotone.ArrowBack
import androidx.compose.material.icons.twotone.Apps
import androidx.compose.material.icons.twotone.BugReport
import androidx.compose.material.icons.twotone.FolderShared
import androidx.compose.material.icons.twotone.Lock
import androidx.compose.material.icons.twotone.Security
import androidx.compose.material.icons.twotone.Shield
import androidx.compose.material.icons.twotone.Sync
import androidx.compose.material.icons.twotone.VisibilityOff
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.PrimaryTabRow
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.rememberTopAppBarState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.bakasu.bakasu.R
import org.bakasu.bakasu.data.packageinfo.InstalledPackageRepository
import org.bakasu.bakasu.data.shell.KsuCliRepository
import org.bakasu.bakasu.ui.component.SwipeableSnackbarHost
import org.bakasu.bakasu.ui.component.settings.SegmentedColumn
import org.bakasu.bakasu.ui.component.settings.SettingsBaseWidget
import org.bakasu.bakasu.ui.component.settings.SettingsSwitchWidget
import org.bakasu.bakasu.ui.navigation.LocalNavigator
import org.bakasu.bakasu.ui.theme.blurSource
import org.bakasu.bakasu.ui.util.LocalSnackbarHost
import org.bakasu.bakasu.ui.util.adaptiveScaffoldWindowInsets
import org.bakasu.bakasu.ui.util.showReplacingSnackbar
import org.koin.compose.koinInject

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HMAScreen(
    showBackButton: Boolean = true,
    bottomPadding: Dp = 0.dp,
) {
    val navigator = LocalNavigator.current
    val ksuCli = koinInject<KsuCliRepository>()
    val packageRepo = koinInject<InstalledPackageRepository>()
    val installedPackages by packageRepo.packages.collectAsState()
    val snackBarHost = LocalSnackbarHost.current
    val scope = rememberCoroutineScope()
    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior(rememberTopAppBarState())

    var selectedTab by remember { mutableIntStateOf(0) }
    var hmaStatus by remember { mutableStateOf(KsuCliRepository.HmaStatus()) }
    var hmaConfig by remember { mutableStateOf(KsuCliRepository.HmaConfig()) }
    var hmaPathsConfig by remember { mutableStateOf(KsuCliRepository.HmaPathsConfig()) }
    var zygiskInfo by remember { mutableStateOf(KsuCliRepository.ZygiskInfo()) }

    fun refreshAll() {
        scope.launch(Dispatchers.IO) {
            val status = ksuCli.getHmaStatus()
            val config = ksuCli.getHmaConfig()
            val paths = ksuCli.getHmaPathsConfig()
            val zInfo = ksuCli.getZygiskInfo()
            withContext(Dispatchers.Main) {
                if (status != null) hmaStatus = status
                if (config != null) hmaConfig = config
                if (paths != null) hmaPathsConfig = paths
                zygiskInfo = zInfo
            }
        }
    }

    fun updateConfigOption(key: String, value: Boolean, label: String) {
        scope.launch(Dispatchers.IO) {
            val ok = ksuCli.setHmaConfigOption(key, value)
            snackBarHost.showReplacingSnackbar(
                if (ok) "$label ${if (value) "enabled" else "disabled"}" else "Failed to update $label",
            )
            refreshAll()
        }
    }

    LaunchedEffect(Unit) {
        refreshAll()
        if (installedPackages.isEmpty()) {
            packageRepo.refresh()
        }
    }

    val tabs = listOf("Settings", "Scope", "Templates", "Paths")

    Scaffold(
        topBar = {
            Column(
                modifier = Modifier.background(MaterialTheme.colorScheme.surface),
            ) {
                TopAppBar(
                    title = { Text(stringResource(R.string.settings_hma_title)) },
                    navigationIcon = {
                        if (showBackButton) {
                            IconButton(onClick = { navigator.pop() }) {
                                Icon(
                                    imageVector = Icons.AutoMirrored.TwoTone.ArrowBack,
                                    contentDescription = null,
                                )
                            }
                        }
                    },
                    colors = TopAppBarDefaults.topAppBarColors(
                        containerColor = MaterialTheme.colorScheme.surface,
                    ),
                )
                PrimaryTabRow(
                    selectedTabIndex = selectedTab,
                    containerColor = MaterialTheme.colorScheme.surface,
                ) {
                    tabs.forEachIndexed { index, title ->
                        Tab(
                            selected = selectedTab == index,
                            onClick = { selectedTab = index },
                            text = { Text(title) },
                        )
                    }
                }
            }
        },
        snackbarHost = {
            SwipeableSnackbarHost(hostState = snackBarHost)
        },
        containerColor = Color.Transparent,
        contentColor = MaterialTheme.colorScheme.onSurface,
        contentWindowInsets = adaptiveScaffoldWindowInsets(includeBottom = false),
    ) { innerPadding ->
        val listPadding = PaddingValues(
            top = innerPadding.calculateTopPadding() + 5.dp,
            bottom = innerPadding.calculateBottomPadding() + 20.dp + bottomPadding,
        )

        when (selectedTab) {
            0 -> HmaOverviewTab(
                hmaStatus = hmaStatus,
                hmaConfig = hmaConfig,
                zygiskInfo = zygiskInfo,
                contentPadding = listPadding,
                modifier = Modifier
                    .fillMaxSize()
                    .nestedScroll(scrollBehavior.nestedScrollConnection)
                    .blurSource(),
                onToggleHma = { checked ->
                    hmaStatus = hmaStatus.copy(enabled = checked)
                    scope.launch(Dispatchers.IO) {
                        val ok = ksuCli.setHmaEnabled(checked)
                        snackBarHost.showReplacingSnackbar(
                            if (ok) {
                                if (checked) "HMA OSS + UID enabled" else "HMA OSS + UID disabled"
                            } else {
                                "Failed to toggle HMA"
                            },
                        )
                        refreshAll()
                    }
                },
                onSyncRules = {
                    scope.launch(Dispatchers.IO) {
                        val ok = ksuCli.syncHmaUidRules()
                        snackBarHost.showReplacingSnackbar(
                            if (ok) "HMA hybrid rules synchronized" else "Failed to sync HMA rules",
                        )
                        refreshAll()
                    }
                },
                onToggleConfig = { key, value, label ->
                    hmaConfig = when (key) {
                        "uidFilterEnabled" -> hmaConfig.copy(uidFilterEnabled = value)
                        "forceMountData" -> hmaConfig.copy(forceMountData = value)
                        "skipSystemAppDataIsolation" -> hmaConfig.copy(skipSystemAppDataIsolation = value)
                        "altAppDataIsolation" -> hmaConfig.copy(altAppDataIsolation = value)
                        "altVoldAppDataIsolation" -> hmaConfig.copy(altVoldAppDataIsolation = value)
                        "disableActivityLaunchProtection" -> hmaConfig.copy(disableActivityLaunchProtection = value)
                        "detailLog" -> hmaConfig.copy(detailLog = value)
                        "errorOnlyLog" -> hmaConfig.copy(errorOnlyLog = value)
                        else -> hmaConfig
                    }
                    updateConfigOption(key, value, label)
                },
            )

            1 -> HmaAppScopeTab(
                config = hmaConfig,
                installedPackages = installedPackages,
                contentPadding = listPadding,
                onUpdateScope = { enabled, updatedScope ->
                    scope.launch(Dispatchers.IO) {
                        val ok = if (enabled) {
                            ksuCli.setHmaAppScope(updatedScope)
                        } else {
                            ksuCli.removeHmaAppScope(updatedScope.packageName)
                        }
                        snackBarHost.showReplacingSnackbar(
                            if (ok) {
                                "Updated scope for ${updatedScope.packageName}"
                            } else {
                                "Failed to update app scope"
                            },
                        )
                        refreshAll()
                    }
                },
            )

            2 -> HmaTemplatesTab(
                config = hmaConfig,
                installedPackages = installedPackages,
                contentPadding = listPadding,
                onSaveTemplate = { name, isWhitelist, apps ->
                    scope.launch(Dispatchers.IO) {
                        val ok = ksuCli.setHmaTemplate(name, isWhitelist, apps)
                        snackBarHost.showReplacingSnackbar(
                            if (ok) "Template '$name' saved" else "Failed to save template",
                        )
                        refreshAll()
                    }
                },
                onDeleteTemplate = { name ->
                    scope.launch(Dispatchers.IO) {
                        val ok = ksuCli.deleteHmaTemplate(name)
                        snackBarHost.showReplacingSnackbar(
                            if (ok) "Template '$name' deleted" else "Failed to delete template",
                        )
                        refreshAll()
                    }
                },
                onReloadPresets = {
                    scope.launch(Dispatchers.IO) {
                        val ok = ksuCli.syncHmaUidRules()
                        snackBarHost.showReplacingSnackbar(
                            if (ok) "Official HMA presets reloaded" else "Failed to reload presets",
                        )
                        refreshAll()
                    }
                },
            )

            3 -> HmaPathsTab(
                pathsConfig = hmaPathsConfig,
                installedPackages = installedPackages,
                contentPadding = listPadding,
                onToggleAutoHide = { enabled ->
                    hmaPathsConfig = hmaPathsConfig.copy(autoHideAppPaths = enabled)
                    scope.launch(Dispatchers.IO) {
                        val ok = ksuCli.setHmaAutoHideAppPaths(enabled)
                        snackBarHost.showReplacingSnackbar(
                            if (ok) {
                                "Auto-hide app paths ${if (enabled) "enabled" else "disabled"}"
                            } else {
                                "Failed to update auto-hide setting"
                            },
                        )
                        refreshAll()
                    }
                },
                onSaveGlobalPaths = { paths ->
                    scope.launch(Dispatchers.IO) {
                        val ok = ksuCli.setHmaGlobalPaths(paths)
                        snackBarHost.showReplacingSnackbar(
                            if (ok) "Global hidden paths updated" else "Failed to update global paths",
                        )
                        refreshAll()
                    }
                },
                onSaveAppPaths = { pkg, paths ->
                    scope.launch(Dispatchers.IO) {
                        val ok = ksuCli.setHmaAppPaths(pkg, paths)
                        snackBarHost.showReplacingSnackbar(
                            if (ok) "Updated hidden paths for $pkg" else "Failed to update paths for $pkg",
                        )
                        refreshAll()
                    }
                },
            )
        }
    }
}

@Composable
private fun HmaOverviewTab(
    hmaStatus: KsuCliRepository.HmaStatus,
    hmaConfig: KsuCliRepository.HmaConfig,
    zygiskInfo: KsuCliRepository.ZygiskInfo,
    contentPadding: PaddingValues,
    modifier: Modifier = Modifier,
    onToggleHma: (Boolean) -> Unit,
    onSyncRules: () -> Unit,
    onToggleConfig: (key: String, value: Boolean, label: String) -> Unit,
) {
    val version = hmaStatus.version.ifEmpty { "177-UID3.1" }
    val gen = hmaStatus.active_generation.ifEmpty { "g0" }

    LazyColumn(
        modifier = modifier.padding(horizontal = 0.dp),
        contentPadding = contentPadding,
    ) {
        item {
            SegmentedColumn(
                title = "HMA & UID Control",
                content = {
                    item {
                        SettingsSwitchWidget(
                            icon = Icons.TwoTone.VisibilityOff,
                            title = stringResource(R.string.hma_enable_title),
                            description = stringResource(R.string.hma_enable_summary),
                            checked = hmaStatus.enabled,
                            onCheckedChange = onToggleHma,
                        )
                    }
                    item {
                        SettingsSwitchWidget(
                            icon = Icons.TwoTone.Security,
                            title = "UID Seccomp Filter",
                            description = "Isolate hidden UIDs at kernel seccomp level (HMA-OSS-UID)",
                            checked = hmaConfig.uidFilterEnabled,
                            onCheckedChange = { onToggleConfig("uidFilterEnabled", it, "UID Seccomp Filter") },
                        )
                    }
                    item {
                        SettingsBaseWidget(
                            icon = Icons.TwoTone.Sync,
                            title = stringResource(R.string.hma_sync_action),
                            description = stringResource(R.string.hma_sync_summary),
                            onClick = { onSyncRules() },
                        )
                    }
                },
            )
        }

        item {
            SegmentedColumn(
                title = "Data Isolation & Protection",
                content = {
                    item {
                        SettingsSwitchWidget(
                            icon = Icons.TwoTone.FolderShared,
                            title = "Force Mount Data",
                            description = "Enforce Android 11+ scoped app data isolation",
                            checked = hmaConfig.forceMountData,
                            onCheckedChange = { onToggleConfig("forceMountData", it, "Force Mount Data") },
                        )
                    }
                    item {
                        SettingsSwitchWidget(
                            icon = Icons.TwoTone.Shield,
                            title = "Skip System App Isolation",
                            description = "Avoid bootloops by skipping vold isolation for system apps",
                            checked = hmaConfig.skipSystemAppDataIsolation,
                            onCheckedChange = {
                                onToggleConfig("skipSystemAppDataIsolation", it, "Skip System App Isolation")
                            },
                        )
                    }
                    item {
                        SettingsSwitchWidget(
                            icon = Icons.TwoTone.FolderShared,
                            title = "Alternative AppData Isolation",
                            description = "Use propless appdata isolation mechanism",
                            checked = hmaConfig.altAppDataIsolation,
                            onCheckedChange = {
                                onToggleConfig("altAppDataIsolation", it, "Alternative AppData Isolation")
                            },
                        )
                    }
                    item {
                        SettingsSwitchWidget(
                            icon = Icons.TwoTone.FolderShared,
                            title = "Alternative Vold Isolation",
                            description = "Use propless vold appdata isolation",
                            checked = hmaConfig.altVoldAppDataIsolation,
                            onCheckedChange = {
                                onToggleConfig("altVoldAppDataIsolation", it, "Alternative Vold Isolation")
                            },
                        )
                    }
                    item {
                        SettingsSwitchWidget(
                            icon = Icons.TwoTone.Lock,
                            title = "Activity Launch Protection",
                            description = "Block target apps from launching hidden apps",
                            checked = !hmaConfig.disableActivityLaunchProtection,
                            onCheckedChange = { enabled ->
                                onToggleConfig("disableActivityLaunchProtection", !enabled, "Activity Launch Protection")
                            },
                        )
                    }
                },
            )
        }

        item {
            SegmentedColumn(
                title = "Logging",
                content = {
                    item {
                        SettingsSwitchWidget(
                            icon = Icons.TwoTone.BugReport,
                            title = "Detailed Logging",
                            description = "Enable verbose HMA hook diagnostics",
                            checked = hmaConfig.detailLog,
                            onCheckedChange = { onToggleConfig("detailLog", it, "Detailed Logging") },
                        )
                    }
                    item {
                        SettingsSwitchWidget(
                            icon = Icons.TwoTone.BugReport,
                            title = "Error-Only Logging",
                            description = "Restrict HMA log channels to errors only",
                            checked = hmaConfig.errorOnlyLog,
                            onCheckedChange = { onToggleConfig("errorOnlyLog", it, "Error-Only Logging") },
                        )
                    }
                },
            )
        }

        item {
            SegmentedColumn(
                title = stringResource(R.string.hma_status_card_title),
                content = {
                    item {
                        val engineName = when {
                            zygiskInfo.state == "running" && zygiskInfo.name.isNotEmpty() ->
                                "${zygiskInfo.name} (${zygiskInfo.version}) • HMA ($version)"

                            hmaStatus.installed -> "Zygisk HMA Module ($version)"

                            else -> stringResource(R.string.hma_status_disabled)
                        }
                        SettingsBaseWidget(
                            icon = Icons.TwoTone.Shield,
                            title = stringResource(R.string.hma_engine_status),
                            description = engineName,
                        )
                    }
                    item {
                        SettingsBaseWidget(
                            icon = Icons.TwoTone.Security,
                            title = stringResource(R.string.hma_uid_filter_status),
                            description = if (hmaStatus.uid_scope_ready) {
                                "${stringResource(R.string.hma_status_ready)} ($gen)"
                            } else if (hmaStatus.enabled) {
                                stringResource(R.string.hma_status_active)
                            } else {
                                stringResource(R.string.hma_status_disabled)
                            },
                        )
                    }
                    item {
                        SettingsBaseWidget(
                            icon = Icons.TwoTone.Apps,
                            title = stringResource(R.string.hma_scoped_apps_count),
                            description = "${hmaStatus.scoped_apps_count} apps active in UID seccomp • ${hmaStatus.root_granted_hidden_count} root UIDs isolated",
                        )
                    }
                },
            )
        }

        item {
            val st = hmaConfig.settingsTemplates
            val accessibilityCount = st["accessibility"]?.size ?: 0
            val inputMethodCount = st["input_method"]?.size ?: st["inputMethod"]?.size ?: st["ime"]?.size ?: 0
            val devOptsCount = st["dev_options"]?.size ?: st["developerOptions"]?.size ?: st["development"]?.size ?: 0
            SegmentedColumn(
                title = "System Settings Presets",
                content = {
                    item {
                        SettingsBaseWidget(
                            icon = Icons.TwoTone.Security,
                            title = "Accessibility Services",
                            description = if (accessibilityCount > 0) "$accessibilityCount rules active (accessibility_enabled, services)" else "Not configured",
                        )
                    }
                    item {
                        SettingsBaseWidget(
                            icon = Icons.TwoTone.Security,
                            title = "Input Methods (IME)",
                            description = if (inputMethodCount > 0) "$inputMethodCount rules active (default_input_method, tts_default_synth)" else "Not configured",
                        )
                    }
                    item {
                        SettingsBaseWidget(
                            icon = Icons.TwoTone.Security,
                            title = "Developer Options",
                            description = if (devOptsCount > 0) "$devOptsCount rules active (adb, dev_settings, mock_location, hidden_api)" else "Not configured",
                        )
                    }
                },
            )
        }
    }
}
