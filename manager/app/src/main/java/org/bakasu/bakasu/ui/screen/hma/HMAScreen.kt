package org.bakasu.bakasu.ui.screen.hma

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.twotone.ArrowBack
import androidx.compose.material.icons.twotone.Apps
import androidx.compose.material.icons.twotone.Security
import androidx.compose.material.icons.twotone.Shield
import androidx.compose.material.icons.twotone.Sync
import androidx.compose.material.icons.twotone.VisibilityOff
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
fun HMAScreen() {
    val navigator = LocalNavigator.current
    val ksuCli = koinInject<KsuCliRepository>()
    val snackBarHost = LocalSnackbarHost.current
    val scope = rememberCoroutineScope()
    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior(rememberTopAppBarState())

    var isHmaEnabled by remember { mutableStateOf(false) }
    var isHmaInstalled by remember { mutableStateOf(false) }
    var hmaVersion by remember { mutableStateOf("177-UID3.1") }
    var isUidScopeReady by remember { mutableStateOf(false) }
    var scopedAppsCount by remember { mutableStateOf(0) }
    var activeGeneration by remember { mutableStateOf("") }
    var isChecking by remember { mutableStateOf(true) }

    fun refreshStatus() {
        scope.launch(Dispatchers.IO) {
            isChecking = true
            val status = ksuCli.getHmaStatus()
            withContext(Dispatchers.Main) {
                if (status != null) {
                    isHmaEnabled = status.enabled
                    isHmaInstalled = status.installed
                    hmaVersion = status.version.ifEmpty { "177-UID3.1" }
                    isUidScopeReady = status.uid_scope_ready
                    scopedAppsCount = status.scoped_apps_count
                    activeGeneration = status.active_generation
                }
                isChecking = false
            }
        }
    }

    LaunchedEffect(Unit) {
        refreshStatus()
    }

    Scaffold(
        topBar = {
            LargeFlexibleTopAppBar(
                title = { Text(stringResource(R.string.settings_hma_title)) },
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
                    title = "HMA & UID Control",
                    content = {
                        item {
                            SettingsSwitchWidget(
                                icon = Icons.TwoTone.VisibilityOff,
                                title = stringResource(R.string.hma_enable_title),
                                description = stringResource(R.string.hma_enable_summary),
                                checked = isHmaEnabled,
                                onCheckedChange = { checked ->
                                    isHmaEnabled = checked
                                    scope.launch(Dispatchers.IO) {
                                        val ok = ksuCli.setHmaEnabled(checked)
                                        snackBarHost.showReplacingSnackbar(
                                            if (ok) {
                                                if (checked) "HMA OSS + UID enabled" else "HMA OSS + UID disabled"
                                            } else {
                                                "Failed to toggle HMA"
                                            },
                                        )
                                        refreshStatus()
                                    }
                                },
                            )
                        }

                        item {
                            SettingsBaseWidget(
                                icon = Icons.TwoTone.Sync,
                                title = stringResource(R.string.hma_sync_action),
                                description = stringResource(R.string.hma_sync_summary),
                                onClick = {
                                    scope.launch(Dispatchers.IO) {
                                        val ok = ksuCli.syncHmaUidRules()
                                        snackBarHost.showReplacingSnackbar(
                                            if (ok) "HMA UID rules synchronized successfully" else "Failed to sync HMA UID rules",
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
                    title = stringResource(R.string.hma_status_card_title),
                    content = {
                        item {
                            SettingsBaseWidget(
                                icon = Icons.TwoTone.Shield,
                                title = stringResource(R.string.hma_engine_status),
                                description = if (isHmaInstalled) {
                                    "Built-in Zygisk Module ($hmaVersion)"
                                } else {
                                    stringResource(R.string.hma_status_disabled)
                                },
                            )
                        }

                        item {
                            SettingsBaseWidget(
                                icon = Icons.TwoTone.Security,
                                title = stringResource(R.string.hma_uid_filter_status),
                                description = if (isUidScopeReady) {
                                    "${stringResource(R.string.hma_status_ready)} (${if (activeGeneration.isNotEmpty()) activeGeneration else "g0"})"
                                } else if (isHmaEnabled) {
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
                                description = "$scopedAppsCount apps configured with UID seccomp filter",
                            )
                        }
                    },
                )
            }
        }
    }
}
