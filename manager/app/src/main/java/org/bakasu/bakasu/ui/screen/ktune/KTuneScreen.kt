package org.bakasu.bakasu.ui.screen.ktune

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.twotone.ArrowBack
import androidx.compose.material.icons.twotone.BatteryChargingFull
import androidx.compose.material.icons.twotone.Memory
import androidx.compose.material.icons.twotone.Power
import androidx.compose.material.icons.twotone.Thermostat
import androidx.compose.material.icons.twotone.Tune
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
fun KTuneScreen() {
    val navigator = LocalNavigator.current
    val ksuCli = koinInject<KsuCliRepository>()
    val snackBarHost = LocalSnackbarHost.current
    val scope = rememberCoroutineScope()
    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior(rememberTopAppBarState())

    var thermalIndex by remember { mutableIntStateOf(0) }
    var isBypassCharging by remember { mutableStateOf(false) }
    var isChargeLimitEnabled by remember { mutableStateOf(false) }
    var chargeLimitPercent by remember { mutableIntStateOf(80) }
    var swappinessIndex by remember { mutableIntStateOf(2) } // default 60

    val thermalProfiles = listOf("Default", "Dynamic", "Gaming", "Performance", "Battery")
    val chargeLimits = listOf("70%", "80%", "85%", "90%", "100%")
    val swappinessValues = listOf("0 (None)", "30 (Light)", "60 (Balanced)", "100 (Aggressive)")

    LaunchedEffect(Unit) {
        withContext(Dispatchers.IO) {
            val status = ksuCli.getKTuneStatusJson()
            if (status.contains("bypass: 1") || status.contains("\"bypass\": true")) {
                isBypassCharging = true
            }
            if (status.contains("charge_limit: 1") || status.contains("\"charge_limit\": true")) {
                isChargeLimitEnabled = true
            }
        }
    }

    Scaffold(
        topBar = {
            LargeFlexibleTopAppBar(
                title = { Text(stringResource(R.string.settings_ktune_title)) },
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
                    title = stringResource(R.string.ktune_thermal_profile),
                    content = {
                        item {
                            SettingsChooseWidget(
                                icon = Icons.TwoTone.Thermostat,
                                title = stringResource(R.string.ktune_thermal_profile),
                                description = "Active profile: ${thermalProfiles[thermalIndex]}",
                                items = thermalProfiles,
                                selectedIndex = thermalIndex,
                                onSelectedIndexChange = { index ->
                                    thermalIndex = index
                                    scope.launch(Dispatchers.IO) {
                                        ksuCli.setKTuneThermal(index)
                                        snackBarHost.showReplacingSnackbar("Applied ${thermalProfiles[index]} thermal profile")
                                    }
                                },
                            )
                        }
                    },
                )
            }

            item {
                SegmentedColumn(
                    title = "Battery & Power Optimization",
                    content = {
                        item {
                            SettingsSwitchWidget(
                                icon = Icons.TwoTone.Power,
                                title = stringResource(R.string.ktune_bypass_charging),
                                description = stringResource(R.string.ktune_bypass_charging_summary),
                                checked = isBypassCharging,
                                onCheckedChange = { checked ->
                                    isBypassCharging = checked
                                    scope.launch(Dispatchers.IO) {
                                        ksuCli.setKTuneBypass(checked)
                                        snackBarHost.showReplacingSnackbar(
                                            if (checked) "Bypass charging enabled" else "Bypass charging disabled",
                                        )
                                    }
                                },
                            )
                        }

                        item {
                            SettingsSwitchWidget(
                                icon = Icons.TwoTone.BatteryChargingFull,
                                title = stringResource(R.string.ktune_charge_limit),
                                description = stringResource(R.string.ktune_charge_limit_summary),
                                checked = isChargeLimitEnabled,
                                onCheckedChange = { checked ->
                                    isChargeLimitEnabled = checked
                                    scope.launch(Dispatchers.IO) {
                                        ksuCli.setKTuneChargeLimitToggle(checked)
                                        if (checked) ksuCli.setKTuneChargeLimit(chargeLimitPercent)
                                        snackBarHost.showReplacingSnackbar(
                                            if (checked) "Charge limit set to $chargeLimitPercent%" else "Charge limit disabled",
                                        )
                                    }
                                },
                            )
                        }

                        item(visible = isChargeLimitEnabled) {
                            SettingsChooseWidget(
                                icon = Icons.TwoTone.BatteryChargingFull,
                                title = "Stop Threshold",
                                description = "Halt charging when reaching limit",
                                items = chargeLimits,
                                selectedIndex = when (chargeLimitPercent) {
                                    70 -> 0
                                    80 -> 1
                                    85 -> 2
                                    90 -> 3
                                    else -> 4
                                },
                                onSelectedIndexChange = { index ->
                                    val percent = when (index) {
                                        0 -> 70
                                        1 -> 80
                                        2 -> 85
                                        3 -> 90
                                        else -> 100
                                    }
                                    chargeLimitPercent = percent
                                    scope.launch(Dispatchers.IO) {
                                        ksuCli.setKTuneChargeLimit(percent)
                                        snackBarHost.showReplacingSnackbar("Limit set to $percent%")
                                    }
                                },
                            )
                        }
                    },
                )
            }

            item {
                SegmentedColumn(
                    title = "Kernel Memory & Swappiness",
                    content = {
                        item {
                            SettingsChooseWidget(
                                icon = Icons.TwoTone.Memory,
                                title = stringResource(R.string.ktune_memory_swappiness),
                                description = stringResource(R.string.ktune_memory_swappiness_summary),
                                items = swappinessValues,
                                selectedIndex = swappinessIndex,
                                onSelectedIndexChange = { index ->
                                    swappinessIndex = index
                                    val swpVal = when (index) {
                                        0 -> 0
                                        1 -> 30
                                        2 -> 60
                                        else -> 100
                                    }
                                    scope.launch(Dispatchers.IO) {
                                        ksuCli.setKTuneSwappiness(swpVal)
                                        snackBarHost.showReplacingSnackbar("Swappiness set to $swpVal")
                                    }
                                },
                            )
                        }
                    },
                )
            }
        }
    }
}
