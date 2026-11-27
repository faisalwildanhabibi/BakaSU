package org.bakasu.bakasu.ui.screen.zygisk

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.twotone.ArrowBack
import androidx.compose.material.icons.twotone.Extension
import androidx.compose.material.icons.twotone.Refresh
import androidx.compose.material.icons.twotone.Security
import androidx.compose.material.icons.twotone.TaskAlt
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LargeFlexibleTopAppBar
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.rememberTopAppBarState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
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
import org.bakasu.bakasu.data.shell.KsuCliRepository
import org.bakasu.bakasu.ui.component.SwipeableSnackbarHost
import org.bakasu.bakasu.ui.component.settings.SegmentedColumn
import org.bakasu.bakasu.ui.component.settings.SettingsBaseWidget
import org.bakasu.bakasu.ui.component.settings.SettingsChooseWidget
import org.bakasu.bakasu.ui.component.settings.SettingsSwitchWidget
import org.bakasu.bakasu.ui.navigation.LocalNavigator
import org.bakasu.bakasu.ui.theme.blurSource
import org.bakasu.bakasu.ui.util.LocalSnackbarHost
import org.bakasu.bakasu.ui.util.adaptiveScaffoldWindowInsets
import org.bakasu.bakasu.ui.util.showReplacingSnackbar
import org.koin.compose.koinInject

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ZygiskScreen(
    showBackButton: Boolean = true,
    bottomPadding: Dp = 0.dp,
) {
    val navigator = LocalNavigator.current
    val ksuCli = koinInject<KsuCliRepository>()
    val snackBarHost = LocalSnackbarHost.current
    val scope = rememberCoroutineScope()
    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior(rememberTopAppBarState())

    var isBuiltinEnabled by remember { mutableStateOf(false) }
    var zygiskInfo by remember { mutableStateOf(KsuCliRepository.ZygiskInfo()) }
    var zygiskModules by remember { mutableStateOf<List<KsuCliRepository.ZygiskModuleInfo>>(emptyList()) }
    var customLinker by remember { mutableStateOf(false) }
    var anonymousMemory by remember { mutableStateOf(false) }
    var pltEngineIndex by remember { mutableStateOf(0) }
    val pltEngines = remember { listOf("yukiplt", "bytehook", "lsplt") }
    var isChecking by remember { mutableStateOf(true) }

    fun refreshState() {
        scope.launch(Dispatchers.IO) {
            isChecking = true
            val info = ksuCli.getZygiskInfo()
            val modules = ksuCli.getZygiskModules()
            val statusJson = ksuCli.getZygiskStatusJson()
            val rawConfig = ksuCli.getZygiskConfigRaw()
            val builtinEnabled = statusJson.contains("\"enabled\":true") || statusJson.contains("\"enabled\": true")
            val pltVal = rawConfig["plt_engine"].orEmpty().ifEmpty { "yukiplt" }
            withContext(Dispatchers.Main) {
                zygiskInfo = info
                zygiskModules = modules
                isBuiltinEnabled = builtinEnabled
                customLinker = rawConfig["custom_linker"] == "1" || rawConfig["custom_linker"] == "true"
                anonymousMemory = rawConfig["anonymous_memory"] == "1" || rawConfig["anonymous_memory"] == "true"
                pltEngineIndex = pltEngines.indexOf(pltVal).coerceAtLeast(0)
                isChecking = false
            }
        }
    }

    LaunchedEffect(Unit) {
        refreshState()
    }

    val isRunning = zygiskInfo.state == "running"
    val engineSummary = when {
        zygiskInfo.name.isNotEmpty() && zygiskInfo.version.isNotEmpty() -> "${zygiskInfo.name} (${zygiskInfo.version})"
        zygiskInfo.name.isNotEmpty() -> zygiskInfo.name
        else -> "Not Detected"
    }
    val statusSummary = when {
        isChecking -> "Checking..."
        zygiskInfo.state == "running" -> stringResource(R.string.zygisk_status_running)
        zygiskInfo.state == "reboot" -> "Pending Reboot"
        else -> stringResource(R.string.zygisk_status_disabled)
    }

    Scaffold(
        modifier = Modifier
            .fillMaxSize()
            .nestedScroll(scrollBehavior.nestedScrollConnection),
        topBar = {
            LargeFlexibleTopAppBar(
                title = { Text(stringResource(R.string.settings_zygisk_title)) },
                navigationIcon = {
                    if (showBackButton) {
                        IconButton(onClick = { navigator.pop() }) {
                            Icon(Icons.AutoMirrored.TwoTone.ArrowBack, contentDescription = null)
                        }
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = Color.Transparent,
                    scrolledContainerColor = Color.Transparent,
                ),
                scrollBehavior = scrollBehavior,
            )
        },
        snackbarHost = { SwipeableSnackbarHost(hostState = snackBarHost) },
        contentWindowInsets = adaptiveScaffoldWindowInsets(),
    ) { innerPadding ->
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
                .blurSource(),
            contentPadding = PaddingValues(start = 16.dp, top = 8.dp, end = 16.dp, bottom = 16.dp + bottomPadding),
        ) {
            item {
                SegmentedColumn(
                    title = stringResource(R.string.home_zygisk_info),
                    modifier = Modifier.padding(bottom = 12.dp),
                ) {
                    item {
                        SettingsBaseWidget(
                            title = stringResource(R.string.home_zygisk_engine),
                            description = engineSummary,
                            icon = Icons.TwoTone.Security,
                        )
                    }
                    item {
                        SettingsBaseWidget(
                            title = stringResource(R.string.home_zygisk_status),
                            description = statusSummary,
                            icon = Icons.TwoTone.TaskAlt,
                        )
                    }
                    item {
                        SettingsSwitchWidget(
                            title = stringResource(R.string.zygisk_enable_title),
                            description = stringResource(R.string.zygisk_enable_summary),
                            icon = Icons.TwoTone.Security,
                            checked = isBuiltinEnabled,
                            onCheckedChange = { checked ->
                                isBuiltinEnabled = checked
                                scope.launch(Dispatchers.IO) {
                                    val ok = ksuCli.setBuiltinZygiskEnabled(checked)
                                    refreshState()
                                    snackBarHost.showReplacingSnackbar(
                                        if (ok) "Built-in Zygisk ${if (checked) "enabled (reboot to apply)" else "disabled"}" else "Failed to toggle Built-in Zygisk",
                                    )
                                }
                            },
                        )
                    }
                    if (isBuiltinEnabled) {
                        item {
                            SettingsBaseWidget(
                                title = stringResource(R.string.zygisk_rearm_action),
                                description = stringResource(R.string.zygisk_rearm_summary),
                                icon = Icons.TwoTone.Refresh,
                                onClick = {
                                    scope.launch(Dispatchers.IO) {
                                        val ok = ksuCli.rearmZygisk()
                                        refreshState()
                                        snackBarHost.showReplacingSnackbar(
                                            if (ok) "Zygisk rearmed successfully" else "Failed to rearm Zygisk",
                                        )
                                    }
                                },
                            )
                        }
                    }
                }
            }

            if (isBuiltinEnabled) {
                item {
                    SegmentedColumn(
                        title = "Stealth & Loader",
                        modifier = Modifier.padding(bottom = 12.dp),
                    ) {
                        item {
                            SettingsSwitchWidget(
                                title = "Custom ELF Linker",
                                description = "Load modules directly into memory without entering system solist",
                                icon = Icons.TwoTone.Security,
                                checked = customLinker,
                                onCheckedChange = { checked ->
                                    customLinker = checked
                                    scope.launch(Dispatchers.IO) {
                                        val ok = ksuCli.setZygiskConfig("custom_linker", checked)
                                        snackBarHost.showReplacingSnackbar(
                                            if (ok) "Custom ELF Linker ${if (checked) "enabled" else "disabled"}" else "Failed to update custom_linker",
                                        )
                                    }
                                },
                            )
                        }
                        item {
                            SettingsSwitchWidget(
                                title = "Anonymous Memory",
                                description = "Convert module memory mappings in /proc/self/maps to anonymous",
                                icon = Icons.TwoTone.Security,
                                checked = anonymousMemory,
                                onCheckedChange = { checked ->
                                    anonymousMemory = checked
                                    scope.launch(Dispatchers.IO) {
                                        val ok = ksuCli.setZygiskConfig("anonymous_memory", checked)
                                        snackBarHost.showReplacingSnackbar(
                                            if (ok) "Anonymous Memory ${if (checked) "enabled" else "disabled"}" else "Failed to update anonymous_memory",
                                        )
                                    }
                                },
                            )
                        }
                        item {
                            SettingsChooseWidget(
                                title = "PLT Hook Engine",
                                items = pltEngines,
                                selectedIndex = pltEngineIndex,
                                icon = Icons.TwoTone.Security,
                                onSelectedIndexChange = { idx ->
                                    pltEngineIndex = idx
                                    val engine = pltEngines[idx]
                                    scope.launch(Dispatchers.IO) {
                                        val ok = ksuCli.setZygiskConfigValue("plt_engine", engine)
                                        snackBarHost.showReplacingSnackbar(
                                            if (ok) "PLT Engine set to $engine" else "Failed to set plt_engine",
                                        )
                                    }
                                },
                            )
                        }
                    }
                }
            }

            item {
                SegmentedColumn(
                    title = "${stringResource(R.string.home_zygisk_modules)} (${zygiskModules.size})",
                    modifier = Modifier.padding(bottom = 12.dp),
                ) {
                    if (zygiskModules.isEmpty()) {
                        item {
                            SettingsBaseWidget(
                                title = "No Zygisk modules loaded",
                                description = "Install a Zygisk module or enable Zygisk to load modules",
                                icon = Icons.TwoTone.Extension,
                            )
                        }
                    } else {
                        zygiskModules.forEach { mod ->
                            item {
                                val abiText = if (mod.abi.isNotEmpty()) mod.abi.joinToString("/") else "unknown"
                                SettingsBaseWidget(
                                    title = mod.name.ifEmpty { mod.id },
                                    description = "${mod.version} • ABI: $abiText • ${mod.state}",
                                    icon = Icons.TwoTone.Extension,
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}
