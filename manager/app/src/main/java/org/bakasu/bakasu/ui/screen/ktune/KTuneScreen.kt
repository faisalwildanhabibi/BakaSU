package org.bakasu.bakasu.ui.screen.ktune

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.twotone.ArrowBack
import androidx.compose.material.icons.twotone.BatteryChargingFull
import androidx.compose.material.icons.twotone.Bolt
import androidx.compose.material.icons.twotone.Memory
import androidx.compose.material.icons.twotone.NetworkCheck
import androidx.compose.material.icons.twotone.Power
import androidx.compose.material.icons.twotone.Save
import androidx.compose.material.icons.twotone.Speed
import androidx.compose.material.icons.twotone.Storage
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
import androidx.compose.ui.unit.Dp
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

private val THERMAL_PROFILES = listOf(
    -1 to "Not Set (-1)",
    0 to "Disabled (0)",
    10 to "Dynamic (10)",
    11 to "Class 0 (11)",
    15 to "AR VR (15)",
    8 to "In-calls (8)",
    9 to "Game (9)",
    16 to "Game 2 (16)",
    2 to "Extreme (2)",
    12 to "Pubg (12)",
    13 to "Camera (13)",
    14 to "Youtube (14)",
)

private val CHARGE_LIMIT_VALUES = listOf(70, 80, 90, 95, 100)
private val CHARGE_CURRENT_VALUES = listOf(1000000L, 2000000L, 3000000L, 4000000L, 5000000L, 6000000L)
private val SWAPPINESS_VALUES = listOf(10, 30, 60, 100, 130)
private val VFS_CACHE_VALUES = listOf(20, 50, 100, 150, 200)
private val KCOMPRESSD_VALUES = listOf(0, 8, 16, 24, 32)
private val ZRAM_SIZE_GB_VALUES = listOf(2, 3, 4, 6, 8)
private val QDISC_VALUES = listOf("fq", "fq_codel", "pfifo_fast", "cake")

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun KTuneScreen(
    showBackButton: Boolean = true,
    bottomPadding: Dp = 0.dp,
) {
    val navigator = LocalNavigator.current
    val ksuCli = koinInject<KsuCliRepository>()
    val snackBarHost = LocalSnackbarHost.current
    val scope = rememberCoroutineScope()
    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior(rememberTopAppBarState())

    var statusMap by remember { mutableStateOf<Map<String, String>>(emptyMap()) }
    var thermalIdx by remember { mutableIntStateOf(0) }
    var isBypassCharging by remember { mutableStateOf(false) }
    var isChargeLimitEnabled by remember { mutableStateOf(false) }
    var chargeLimitIdx by remember { mutableIntStateOf(1) }
    var isChargeWritable by remember { mutableStateOf(false) }
    var chargeCurrentIdx by remember { mutableIntStateOf(2) }
    var tcpIndex by remember { mutableIntStateOf(0) }
    var tcpAvailable by remember { mutableStateOf(listOf("bbr", "cubic", "reno")) }
    var qdiscIndex by remember { mutableIntStateOf(0) }
    var swappinessIdx by remember { mutableIntStateOf(2) }
    var vfsCacheIdx by remember { mutableIntStateOf(2) }
    var kcompressdIdx by remember { mutableIntStateOf(3) }
    var zramSizeIdx by remember { mutableIntStateOf(2) }
    var zramAlgoIndex by remember { mutableIntStateOf(0) }
    var zramCompAvailable by remember { mutableStateOf(listOf("lz4", "lzo-rle", "zstd")) }
    var persistEnabled by remember { mutableStateOf(false) }

    LaunchedEffect(Unit) {
        withContext(Dispatchers.IO) {
            val map = ksuCli.getKTuneStatus()
            statusMap = map

            val tId = map["thermal_current"]?.toIntOrNull() ?: -1
            thermalIdx = THERMAL_PROFILES.indexOfFirst { it.first == tId }.coerceAtLeast(0)

            isBypassCharging = map["bypass_current"] == "1"
            isChargeLimitEnabled = map["charge_limit_enabled"] == "1"

            val cl = map["charge_limit_percent"]?.toIntOrNull() ?: 80
            chargeLimitIdx = CHARGE_LIMIT_VALUES.indexOfFirst { it == cl }.let { if (it >= 0) it else 1 }

            isChargeWritable = map["charge_writable"] == "1"
            val cc = map["charge_current"]?.toLongOrNull() ?: 3000000L
            chargeCurrentIdx = CHARGE_CURRENT_VALUES.indices.minByOrNull {
                kotlin.math.abs(CHARGE_CURRENT_VALUES[it] - cc)
            } ?: 2

            val tcpList = map["tcp_available"]?.split(',')?.map { it.trim() }?.filter { it.isNotEmpty() }.orEmpty()
            if (tcpList.isNotEmpty()) tcpAvailable = tcpList
            val tcpCur = map["tcp_current"].orEmpty()
            tcpIndex = tcpAvailable.indexOf(tcpCur).coerceAtLeast(0)

            val qdiscCur = map["qdisc_current"].orEmpty()
            qdiscIndex = QDISC_VALUES.indexOf(qdiscCur).coerceAtLeast(0)

            val swap = map["mem_swappiness"]?.toIntOrNull() ?: 60
            swappinessIdx = SWAPPINESS_VALUES.indices.minByOrNull {
                kotlin.math.abs(SWAPPINESS_VALUES[it] - swap)
            } ?: 2

            val vfs = map["mem_vfs_cache"]?.toIntOrNull() ?: 100
            vfsCacheIdx = VFS_CACHE_VALUES.indices.minByOrNull {
                kotlin.math.abs(VFS_CACHE_VALUES[it] - vfs)
            } ?: 2

            val kcomp = map["mem_kcompressd"]?.toIntOrNull() ?: 24
            kcompressdIdx = KCOMPRESSD_VALUES.indices.minByOrNull {
                kotlin.math.abs(KCOMPRESSD_VALUES[it] - kcomp)
            } ?: 3

            val zramGb = kotlin.math.round((map["zram_disksize"]?.toLongOrNull() ?: 0L).toDouble() / (1024.0 * 1024.0 * 1024.0)).toInt()
            zramSizeIdx = ZRAM_SIZE_GB_VALUES.indices.minByOrNull {
                kotlin.math.abs(ZRAM_SIZE_GB_VALUES[it] - zramGb)
            } ?: 2

            val zramAlgos = map["zram_comp_available"]?.split(',')?.map { it.trim() }?.filter { it.isNotEmpty() }.orEmpty()
            if (zramAlgos.isNotEmpty()) zramCompAvailable = zramAlgos
            val zramCur = map["zram_comp_current"].orEmpty()
            zramAlgoIndex = zramCompAvailable.indexOf(zramCur).coerceAtLeast(0)

            persistEnabled = map["persist"] == "1"
        }
    }

    val thermalSupported = statusMap["thermal_supported"] != "0"
    val bypassSupported = statusMap["bypass_supported"] != "0"
    val chargeLimitSupported = statusMap["charge_limit_supported"] != "0"
    val chargeCurrentSupported = statusMap["charge_current_supported"] != "0"
    val swappinessRo = statusMap["mem_swappiness_ro"] == "1"
    val zramSupported = statusMap["zram_supported"] != "0"

    Scaffold(
        modifier = Modifier
            .fillMaxSize()
            .nestedScroll(scrollBehavior.nestedScrollConnection),
        topBar = {
            LargeFlexibleTopAppBar(
                title = { Text(stringResource(R.string.settings_ktune_title)) },
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
            if (thermalSupported) {
                item {
                    SegmentedColumn(
                        title = stringResource(R.string.ktune_thermal_profile),
                        modifier = Modifier.padding(bottom = 12.dp),
                    ) {
                        item {
                            SettingsChooseWidget(
                                title = stringResource(R.string.ktune_thermal_profile),
                                items = THERMAL_PROFILES.map { it.second },
                                selectedIndex = thermalIdx,
                                icon = Icons.TwoTone.Thermostat,
                                onSelectedIndexChange = { idx ->
                                    thermalIdx = idx
                                    val profileId = THERMAL_PROFILES[idx].first
                                    scope.launch(Dispatchers.IO) {
                                        val ok = ksuCli.setKTuneThermal(profileId)
                                        snackBarHost.showReplacingSnackbar(
                                            if (ok) "Thermal profile: ${THERMAL_PROFILES[idx].second}" else "Failed to set thermal profile",
                                        )
                                    }
                                },
                            )
                        }
                    }
                }
            }

            if (bypassSupported || chargeLimitSupported || chargeCurrentSupported) {
                item {
                    SegmentedColumn(
                        title = "Power & Charging",
                        modifier = Modifier.padding(bottom = 12.dp),
                    ) {
                        if (bypassSupported) {
                            item {
                                SettingsSwitchWidget(
                                    title = stringResource(R.string.ktune_bypass_charging),
                                    description = stringResource(R.string.ktune_bypass_charging_summary),
                                    icon = Icons.TwoTone.Power,
                                    checked = isBypassCharging,
                                    onCheckedChange = { checked ->
                                        isBypassCharging = checked
                                        scope.launch(Dispatchers.IO) {
                                            val ok = ksuCli.setKTuneBypass(checked)
                                            snackBarHost.showReplacingSnackbar(
                                                if (ok) "Bypass charging ${if (checked) "enabled" else "disabled"}" else "Failed to update bypass charging",
                                            )
                                        }
                                    },
                                )
                            }
                        }
                        if (chargeLimitSupported) {
                            item {
                                SettingsSwitchWidget(
                                    title = stringResource(R.string.ktune_charge_limit),
                                    description = stringResource(R.string.ktune_charge_limit_summary),
                                    icon = Icons.TwoTone.BatteryChargingFull,
                                    checked = isChargeLimitEnabled,
                                    onCheckedChange = { checked ->
                                        isChargeLimitEnabled = checked
                                        scope.launch(Dispatchers.IO) {
                                            ksuCli.setKTuneChargeLimitToggle(checked)
                                        }
                                    },
                                )
                            }
                            if (isChargeLimitEnabled) {
                                item {
                                    SettingsChooseWidget(
                                        title = "Charge Limit Threshold",
                                        items = CHARGE_LIMIT_VALUES.map { "$it%" },
                                        selectedIndex = chargeLimitIdx,
                                        icon = Icons.TwoTone.Tune,
                                        onSelectedIndexChange = { idx ->
                                            chargeLimitIdx = idx
                                            val pct = CHARGE_LIMIT_VALUES[idx]
                                            scope.launch(Dispatchers.IO) {
                                                val ok = ksuCli.setKTuneChargeLimit(pct)
                                                snackBarHost.showReplacingSnackbar(
                                                    if (ok) "Charge limit set to $pct%" else "Failed to set charge limit",
                                                )
                                            }
                                        },
                                    )
                                }
                            }
                        }
                        if (chargeCurrentSupported) {
                            item {
                                SettingsSwitchWidget(
                                    title = "Unlock Charge Current Node",
                                    description = "Set charging current sysfs node writable (0644)",
                                    icon = Icons.TwoTone.Bolt,
                                    checked = isChargeWritable,
                                    onCheckedChange = { checked ->
                                        isChargeWritable = checked
                                        scope.launch(Dispatchers.IO) {
                                            val ok = ksuCli.setKTuneChargeWritable(checked)
                                            snackBarHost.showReplacingSnackbar(
                                                if (ok) "Charge current writable: $checked" else "Failed to toggle charge-writable",
                                            )
                                        }
                                    },
                                )
                            }
                            item {
                                SettingsChooseWidget(
                                    title = "Charging Current Limit",
                                    items = CHARGE_CURRENT_VALUES.map { "${it / 1000} mA" },
                                    selectedIndex = chargeCurrentIdx,
                                    enabled = isChargeWritable,
                                    icon = Icons.TwoTone.Bolt,
                                    onSelectedIndexChange = { idx ->
                                        chargeCurrentIdx = idx
                                        val ua = CHARGE_CURRENT_VALUES[idx]
                                        scope.launch(Dispatchers.IO) {
                                            val ok = ksuCli.setKTuneChargeCurrent(ua)
                                            snackBarHost.showReplacingSnackbar(
                                                if (ok) "Charge current limit set to ${ua / 1000} mA" else "Charge current node is read-only",
                                            )
                                        }
                                    },
                                )
                            }
                        }
                    }
                }
            }

            item {
                SegmentedColumn(
                    title = "Network & TCP",
                    modifier = Modifier.padding(bottom = 12.dp),
                ) {
                    item {
                        SettingsChooseWidget(
                            title = "TCP Congestion Algorithm",
                            items = tcpAvailable,
                            selectedIndex = tcpIndex.coerceIn(0, (tcpAvailable.size - 1).coerceAtLeast(0)),
                            icon = Icons.TwoTone.NetworkCheck,
                            onSelectedIndexChange = { idx ->
                                tcpIndex = idx
                                val algo = tcpAvailable[idx]
                                scope.launch(Dispatchers.IO) {
                                    val ok = ksuCli.setKTuneTcpCongestion(algo)
                                    snackBarHost.showReplacingSnackbar(
                                        if (ok) "TCP congestion: $algo" else "Failed to set TCP congestion",
                                    )
                                }
                            },
                        )
                    }
                    item {
                        SettingsChooseWidget(
                            title = "Default Qdisc",
                            items = QDISC_VALUES,
                            selectedIndex = qdiscIndex,
                            icon = Icons.TwoTone.Speed,
                            onSelectedIndexChange = { idx ->
                                qdiscIndex = idx
                                val q = QDISC_VALUES[idx]
                                scope.launch(Dispatchers.IO) {
                                    val ok = ksuCli.setKTuneQdisc(q)
                                    snackBarHost.showReplacingSnackbar(
                                        if (ok) "Qdisc: $q" else "Failed to set Qdisc",
                                    )
                                }
                            },
                        )
                    }
                }
            }

            item {
                SegmentedColumn(
                    title = "Memory & VM",
                    modifier = Modifier.padding(bottom = 12.dp),
                ) {
                    item {
                        SettingsChooseWidget(
                            title = stringResource(R.string.ktune_memory_swappiness),
                            items = SWAPPINESS_VALUES.map { it.toString() },
                            selectedIndex = swappinessIdx,
                            enabled = !swappinessRo,
                            icon = Icons.TwoTone.Memory,
                            onSelectedIndexChange = { idx ->
                                swappinessIdx = idx
                                val value = SWAPPINESS_VALUES[idx]
                                scope.launch(Dispatchers.IO) {
                                    val ok = ksuCli.setKTuneSwappiness(value)
                                    snackBarHost.showReplacingSnackbar(
                                        if (ok) "Swappiness set to $value" else "Failed to update swappiness",
                                    )
                                }
                            },
                        )
                    }
                    item {
                        SettingsChooseWidget(
                            title = "VFS Cache Pressure",
                            items = VFS_CACHE_VALUES.map { it.toString() },
                            selectedIndex = vfsCacheIdx,
                            icon = Icons.TwoTone.Tune,
                            onSelectedIndexChange = { idx ->
                                vfsCacheIdx = idx
                                val value = VFS_CACHE_VALUES[idx]
                                scope.launch(Dispatchers.IO) {
                                    val ok = ksuCli.setKTuneVfsCache(value)
                                    snackBarHost.showReplacingSnackbar(
                                        if (ok) "VFS cache pressure set to $value" else "Failed to set VFS cache pressure",
                                    )
                                }
                            },
                        )
                    }
                    if (statusMap["mem_kcompressd"]?.isNotEmpty() == true) {
                        item {
                            SettingsChooseWidget(
                                title = "kcompressd Threads",
                                items = KCOMPRESSD_VALUES.map { it.toString() },
                                selectedIndex = kcompressdIdx,
                                icon = Icons.TwoTone.Memory,
                                onSelectedIndexChange = { idx ->
                                    kcompressdIdx = idx
                                    val value = KCOMPRESSD_VALUES[idx]
                                    scope.launch(Dispatchers.IO) {
                                        val ok = ksuCli.setKTuneKcompressd(value)
                                        snackBarHost.showReplacingSnackbar(
                                            if (ok) "kcompressd set to $value" else "Failed to set kcompressd",
                                        )
                                    }
                                },
                            )
                        }
                    }
                }
            }

            if (zramSupported) {
                item {
                    SegmentedColumn(
                        title = "ZRAM",
                        modifier = Modifier.padding(bottom = 12.dp),
                    ) {
                        item {
                            SettingsChooseWidget(
                                title = "ZRAM Disk Size",
                                items = ZRAM_SIZE_GB_VALUES.map { "$it GB" },
                                selectedIndex = zramSizeIdx,
                                icon = Icons.TwoTone.Storage,
                                onSelectedIndexChange = { idx ->
                                    zramSizeIdx = idx
                                    val gb = ZRAM_SIZE_GB_VALUES[idx]
                                    scope.launch(Dispatchers.IO) {
                                        val ok = ksuCli.setKTuneZramSize(gb)
                                        snackBarHost.showReplacingSnackbar(
                                            if (ok) "ZRAM size set to $gb GB" else "Failed to set ZRAM size",
                                        )
                                    }
                                },
                            )
                        }
                        item {
                            SettingsChooseWidget(
                                title = "ZRAM Compression Algorithm",
                                items = zramCompAvailable,
                                selectedIndex = zramAlgoIndex.coerceIn(0, (zramCompAvailable.size - 1).coerceAtLeast(0)),
                                icon = Icons.TwoTone.Tune,
                                onSelectedIndexChange = { idx ->
                                    zramAlgoIndex = idx
                                    val algo = zramCompAvailable[idx]
                                    scope.launch(Dispatchers.IO) {
                                        val ok = ksuCli.setKTuneZramAlgorithm(algo)
                                        snackBarHost.showReplacingSnackbar(
                                            if (ok) "ZRAM compression: $algo" else "Failed to set ZRAM compression",
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
                    title = "Persistence",
                    modifier = Modifier.padding(bottom = 12.dp),
                ) {
                    item {
                        SettingsSwitchWidget(
                            title = "Apply Settings on Boot",
                            description = "Persist KTune settings across device reboots",
                            icon = Icons.TwoTone.Save,
                            checked = persistEnabled,
                            onCheckedChange = { checked ->
                                persistEnabled = checked
                                scope.launch(Dispatchers.IO) {
                                    val ok = ksuCli.setKTunePersist(checked)
                                    snackBarHost.showReplacingSnackbar(
                                        if (ok) "Boot persistence ${if (checked) "enabled" else "disabled"}" else "Failed to update persistence",
                                    )
                                }
                            },
                        )
                    }
                }
            }
        }
    }
}
