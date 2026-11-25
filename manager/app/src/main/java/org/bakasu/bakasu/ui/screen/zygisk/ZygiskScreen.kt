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
import androidx.compose.material.icons.twotone.Shield
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
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.bakasu.bakasu.R
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
fun ZygiskScreen() {
    val navigator = LocalNavigator.current
    val ksuCli = koinInject<KsuCliRepository>()
    val snackBarHost = LocalSnackbarHost.current
    val scope = rememberCoroutineScope()
    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior(rememberTopAppBarState())

    var isZygiskEnabled by remember { mutableStateOf(false) }
    var isNomountGuardEnabled by remember { mutableStateOf(true) }
    var activeModulesCount by remember { mutableIntStateOf(0) }
    var isChecking by remember { mutableStateOf(true) }

    fun refreshStatus() {
        scope.launch(Dispatchers.IO) {
            isChecking = true
            isZygiskEnabled = ksuCli.isBuiltinZygiskActive()
            activeModulesCount = ksuCli.getBuiltinZygiskModulesCount()
            isChecking = false
        }
    }

    LaunchedEffect(Unit) {
        refreshStatus()
    }

    Scaffold(
        topBar = {
            LargeFlexibleTopAppBar(
                title = { Text(stringResource(R.string.settings_zygisk_title)) },
                navigationIcon = {
                    IconButton(onClick = { navigator.pop() }) {
                        Icon(
                            imageVector = Icons.AutoMirrored.TwoTone.ArrowBack,
                            contentDescription = null,
                        )
                    }
                },
                scrollBehavior = scrollBehavior,
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = Color.Transparent,
                    scrolledContainerColor = Color.Transparent,
                ),
            )
        },
        snackbarHost = {
            SwipeableSnackbarHost(hostState = snackBarHost)
        },
        containerColor = Color.Transparent,
        contentColor = MaterialTheme.colorScheme.onSurface,
        contentWindowInsets = adaptiveScaffoldWindowInsets(includeBottom = false),
    ) { innerPadding ->
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .nestedScroll(scrollBehavior.nestedScrollConnection)
                .blurSource()
                .padding(horizontal = 0.dp),
            contentPadding = PaddingValues(
                top = innerPadding.calculateTopPadding() + 5.dp,
                bottom = innerPadding.calculateBottomPadding() + 20.dp,
            ),
        ) {
            item {
                SegmentedColumn(
                    title = "Zygisk Engine Control",
                    content = {
                        item {
                            SettingsSwitchWidget(
                                icon = Icons.TwoTone.Extension,
                                title = stringResource(R.string.zygisk_enable_title),
                                description = stringResource(R.string.zygisk_enable_summary),
                                checked = isZygiskEnabled,
                                onCheckedChange = { checked ->
                                    isZygiskEnabled = checked
                                    scope.launch(Dispatchers.IO) {
                                        val ok = ksuCli.setBuiltinZygiskEnabled(checked)
                                        snackBarHost.showReplacingSnackbar(
                                            if (ok) {
                                                if (checked) "Built-in Zygisk enabled" else "Built-in Zygisk disabled"
                                            } else {
                                                "Failed to toggle Zygisk"
                                            },
                                        )
                                        refreshStatus()
                                    }
                                },
                            )
                        }

                        item {
                            SettingsSwitchWidget(
                                icon = Icons.TwoTone.Shield,
                                title = stringResource(R.string.zygisk_nomount_isolation),
                                description = stringResource(R.string.zygisk_nomount_isolation_summary),
                                checked = isNomountGuardEnabled,
                                onCheckedChange = { checked ->
                                    isNomountGuardEnabled = checked
                                    scope.launch(Dispatchers.IO) {
                                        ksuCli.syncZygiskNoMount()
                                        snackBarHost.showReplacingSnackbar(
                                            if (checked) "NoMount isolated guard active" else "NoMount isolated guard disabled",
                                        )
                                    }
                                },
                            )
                        }

                        item {
                            SettingsBaseWidget(
                                icon = Icons.TwoTone.Refresh,
                                title = stringResource(R.string.zygisk_rearm_action),
                                description = stringResource(R.string.zygisk_rearm_summary),
                                onClick = {
                                    scope.launch(Dispatchers.IO) {
                                        val ok = ksuCli.rearmZygisk()
                                        snackBarHost.showReplacingSnackbar(
                                            if (ok) "Zygisk rearmed successfully" else "Failed to rearm Zygisk",
                                        )
                                        refreshStatus()
                                    }
                                },
                            )
                        }
                    },
                )
            }

            item {
                SegmentedColumn(
                    title = "Runtime Telemetry (SukiSU Nova Reference)",
                    content = {
                        item {
                            SettingsBaseWidget(
                                icon = Icons.TwoTone.TaskAlt,
                                title = "Injection Status",
                                description = if (isZygiskEnabled) "Active • libzygisk.so loaded into Zygote" else "Inactive",
                            )
                        }

                        item {
                            SettingsBaseWidget(
                                icon = Icons.TwoTone.Extension,
                                title = "Loaded Zygisk Modules",
                                description = "$activeModulesCount active modules loaded by zygiskd companion",
                            )
                        }

                        item {
                            SettingsBaseWidget(
                                icon = Icons.TwoTone.Security,
                                title = "Stealth & Anti-Detection",
                                description = "TracerPid cloaked via SuSFS • In-memory anonymous map protection",
                            )
                        }
                    },
                )
            }
        }
    }
}
