package org.bakasu.bakasu.ui.screen.ktune

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.twotone.ArrowBack
import androidx.compose.material.icons.twotone.BatteryChargingFull
import androidx.compose.material.icons.twotone.Lock
import androidx.compose.material.icons.twotone.Memory
import androidx.compose.material.icons.twotone.Palette
import androidx.compose.material.icons.twotone.Speed
import androidx.compose.material.icons.twotone.Thermostat
import androidx.compose.material.icons.twotone.Tune
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LargeFlexibleTopAppBar
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Slider
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
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.bakasu.bakasu.data.shell.KsuCliRepository
import org.bakasu.bakasu.ui.component.SwipeableSnackbarHost
import org.bakasu.bakasu.ui.component.settings.SegmentedColumn
import org.bakasu.bakasu.ui.component.settings.SettingsBaseWidget
import org.bakasu.bakasu.ui.navigation.LocalNavigator
import org.bakasu.bakasu.ui.theme.blurSource
import org.bakasu.bakasu.ui.util.LocalSnackbarHost
import org.bakasu.bakasu.ui.util.adaptiveScaffoldWindowInsets
import org.bakasu.bakasu.ui.util.showReplacingSnackbar
import org.koin.compose.koinInject

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun KTuneSubScaffold(
    title: String,
    content: LazyListScope.() -> Unit,
) {
    val navigator = LocalNavigator.current
    val snackBarHost = LocalSnackbarHost.current
    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior(rememberTopAppBarState())

    Scaffold(
        modifier = Modifier
            .fillMaxSize()
            .nestedScroll(scrollBehavior.nestedScrollConnection),
        topBar = {
            LargeFlexibleTopAppBar(
                title = { Text(title) },
                navigationIcon = {
                    IconButton(onClick = { navigator.pop() }) {
                        Icon(Icons.AutoMirrored.TwoTone.ArrowBack, contentDescription = null)
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
            contentPadding = PaddingValues(16.dp),
            content = content,
        )
    }
}

@Composable
fun KTuneBatteryScreen() {
    val ksuCli = koinInject<KsuCliRepository>()
    var details by remember { mutableStateOf<Map<String, String>>(emptyMap()) }

    LaunchedEffect(Unit) {
        while (true) {
            details = withContext(Dispatchers.IO) { ksuCli.getBatteryDetails() }
            delay(2000L)
        }
    }

    val voltageMv = (details["voltage_now"]?.toLongOrNull() ?: 0L) / 1000L
    val currentMa = (details["current_now"]?.toLongOrNull() ?: 0L) / 1000L
    val tempC = (details["temp"]?.toFloatOrNull() ?: 0f) / 10f
    val chargeFullMah = (details["charge_full"]?.toLongOrNull() ?: 0L) / 1000L
    val chargeDesignMah = (details["charge_full_design"]?.toLongOrNull() ?: 0L) / 1000L
    val healthPct = if (chargeDesignMah > 0 && chargeFullMah > 0) (chargeFullMah * 100L / chargeDesignMah) else 0L

    KTuneSubScaffold(title = "Baterai") {
        item {
            SegmentedColumn(title = "Status & Kesehatan Baterai") {
                item {
                    SettingsBaseWidget(
                        title = "Kapasitas & Status",
                        description = "${details["capacity"].orEmpty()}% • ${details["status"].orEmpty()}",
                        icon = Icons.TwoTone.BatteryChargingFull,
                    )
                }
                item {
                    SettingsBaseWidget(
                        title = "Kesehatan & Teknologi",
                        description = "${details["health"].orEmpty()} • ${details["technology"].orEmpty()}",
                        icon = Icons.TwoTone.Tune,
                    )
                }
                item {
                    SettingsBaseWidget(
                        title = "Tegangan & Arus",
                        description = "$voltageMv mV • $currentMa mA",
                        icon = Icons.TwoTone.Speed,
                    )
                }
                item {
                    SettingsBaseWidget(
                        title = "Suhu & Siklus",
                        description = "$tempC °C • Siklus: ${details["cycle_count"].orEmpty().ifEmpty { "N/A" }}",
                        icon = Icons.TwoTone.Thermostat,
                    )
                }
                item {
                    SettingsBaseWidget(
                        title = "Kapasitas Penuh / Desain",
                        description = "$chargeFullMah mAh / $chargeDesignMah mAh" + if (healthPct > 0) " ($healthPct%)" else "",
                        icon = Icons.TwoTone.BatteryChargingFull,
                    )
                }
            }
        }
    }
}

@Composable
fun KTuneThermalMonitorScreen() {
    val ksuCli = koinInject<KsuCliRepository>()
    var zones by remember { mutableStateOf<List<Pair<String, Float>>>(emptyList()) }

    LaunchedEffect(Unit) {
        while (true) {
            zones = withContext(Dispatchers.IO) { ksuCli.getThermalZones() }
            delay(1500L)
        }
    }

    KTuneSubScaffold(title = "Monitor Termal") {
        item {
            SegmentedColumn(title = "Zona Termal (${zones.size})") {
                zones.forEach { (name, temp) ->
                    item {
                        SettingsBaseWidget(
                            title = name,
                            description = String.format("%.1f °C", temp),
                            icon = Icons.TwoTone.Thermostat,
                        )
                    }
                }
            }
        }
    }
}

@Composable
fun KTuneColorCalibrationScreen() {
    val ksuCli = koinInject<KsuCliRepository>()
    val snackBarHost = LocalSnackbarHost.current
    val scope = rememberCoroutineScope()
    var supported by remember { mutableStateOf(false) }
    var r by remember { mutableIntStateOf(256) }
    var g by remember { mutableIntStateOf(256) }
    var b by remember { mutableIntStateOf(256) }
    var sat by remember { mutableIntStateOf(255) }
    var value by remember { mutableIntStateOf(255) }
    var cont by remember { mutableIntStateOf(255) }
    var hue by remember { mutableIntStateOf(0) }

    LaunchedEffect(Unit) {
        withContext(Dispatchers.IO) {
            val state = ksuCli.getKcalState()
            if (state.isNotEmpty() && state["kcal"]?.isNotEmpty() == true) {
                supported = true
                val rgb = state["kcal"].orEmpty().split(' ').mapNotNull { it.toIntOrNull() }
                if (rgb.size >= 3) {
                    r = rgb[0]
                    g = rgb[1]
                    b = rgb[2]
                }
                sat = state["kcal_sat"]?.toIntOrNull() ?: 255
                value = state["kcal_val"]?.toIntOrNull() ?: 255
                cont = state["kcal_cont"]?.toIntOrNull() ?: 255
                hue = state["kcal_hue"]?.toIntOrNull() ?: 0
            }
        }
    }

    KTuneSubScaffold(title = "Kalibrasi Warna") {
        if (!supported) {
            item {
                SegmentedColumn(title = "KCAL") {
                    item {
                        SettingsBaseWidget(
                            title = "KCAL Tidak Tersedia",
                            description = "Kernel tidak mengekspos node /sys/devices/platform/kcal_ctrl.0",
                            icon = Icons.TwoTone.Palette,
                        )
                    }
                }
            }
        } else {
            item {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    listOf(
                        Triple("Red", r, 0..256) to { v: Int -> r = v },
                        Triple("Green", g, 0..256) to { v: Int -> g = v },
                        Triple("Blue", b, 0..256) to { v: Int -> b = v },
                        Triple("Saturasi", sat, 224..383) to { v: Int -> sat = v },
                        Triple("Nilai", value, 128..383) to { v: Int -> value = v },
                        Triple("Kontras", cont, 128..383) to { v: Int -> cont = v },
                        Triple("Hue", hue, 0..1536) to { v: Int -> hue = v },
                    ).forEach { (meta, setter) ->
                        Text("${meta.first}: ${meta.second}", style = MaterialTheme.typography.bodyMedium)
                        Slider(
                            value = meta.second.toFloat(),
                            onValueChange = { setter(it.toInt()) },
                            valueRange = meta.third.first.toFloat()..meta.third.last.toFloat(),
                        )
                    }
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        Button(
                            onClick = {
                                scope.launch(Dispatchers.IO) {
                                    val ok = ksuCli.setKcalValues(r, g, b, sat, value, cont, hue)
                                    snackBarHost.showReplacingSnackbar(if (ok) "Kalibrasi diterapkan" else "Gagal menerapkan KCAL")
                                }
                            },
                            modifier = Modifier.weight(1f),
                        ) {
                            Text("Terapkan")
                        }
                        OutlinedButton(
                            onClick = {
                                r = 256
                                g = 256
                                b = 256
                                sat = 255
                                value = 255
                                cont = 255
                                hue = 0
                                scope.launch(Dispatchers.IO) {
                                    ksuCli.setKcalValues(256, 256, 256, 255, 255, 255, 0)
                                }
                            },
                            modifier = Modifier.weight(1f),
                        ) {
                            Text("Atur ulang")
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun KTuneCpuMonitorScreen() {
    val ksuCli = koinInject<KsuCliRepository>()
    var cores by remember { mutableStateOf<List<KsuCliRepository.CpuCoreInfo>>(emptyList()) }

    LaunchedEffect(Unit) {
        while (true) {
            cores = withContext(Dispatchers.IO) { ksuCli.getCpuCoresInfo() }
            delay(1500L)
        }
    }

    KTuneSubScaffold(title = "Monitor CPU") {
        item {
            SegmentedColumn(title = "Frekuensi Inti CPU (${cores.size})") {
                cores.forEach { core ->
                    item {
                        SettingsBaseWidget(
                            title = "CPU ${core.coreIndex} (${core.governor})",
                            description = "Saat ini: ${core.curFreqKhz / 1000} MHz • Min/Max: ${core.minFreqKhz / 1000}/${core.maxFreqKhz / 1000} MHz",
                            icon = Icons.TwoTone.Memory,
                        )
                    }
                }
            }
        }
    }
}

@Composable
fun KTuneGpuMonitorScreen() {
    val ksuCli = koinInject<KsuCliRepository>()
    var gpu by remember { mutableStateOf<Map<String, String>>(emptyMap()) }

    LaunchedEffect(Unit) {
        while (true) {
            gpu = withContext(Dispatchers.IO) { ksuCli.getGpuInfo() }
            delay(1500L)
        }
    }

    KTuneSubScaffold(title = "Monitor GPU") {
        item {
            SegmentedColumn(title = "Adreno / KGSL Telemetry") {
                item {
                    SettingsBaseWidget(
                        title = "Penggunaan GPU",
                        description = gpu["busy"].orEmpty().ifEmpty { "N/A" },
                        icon = Icons.TwoTone.Speed,
                    )
                }
                item {
                    SettingsBaseWidget(
                        title = "Kecepatan Clock",
                        description = "${gpu["clock"].orEmpty().ifEmpty { "0" }} MHz • Governor: ${gpu["gov"].orEmpty()}",
                        icon = Icons.TwoTone.Memory,
                    )
                }
                item {
                    SettingsBaseWidget(
                        title = "Frekuensi Tersedia",
                        description = gpu["freqs"].orEmpty().ifEmpty { "N/A" },
                        icon = Icons.TwoTone.Tune,
                    )
                }
            }
        }
    }
}

@Composable
fun KTuneWakelockScreen() {
    val ksuCli = koinInject<KsuCliRepository>()
    var sources by remember { mutableStateOf<List<KsuCliRepository.WakeupSourceInfo>>(emptyList()) }

    LaunchedEffect(Unit) {
        sources = withContext(Dispatchers.IO) { ksuCli.getWakeupSources() }
    }

    KTuneSubScaffold(title = "Wakelock") {
        item {
            SegmentedColumn(title = "Wake Sources (${sources.size})") {
                sources.forEach { wl ->
                    item {
                        SettingsBaseWidget(
                            title = wl.name,
                            description = "Total: ${wl.totalTimeMs} ms • Max: ${wl.maxTimeMs} ms • Count: ${wl.activeCount}",
                            icon = Icons.TwoTone.Lock,
                        )
                    }
                }
            }
        }
    }
}

@Composable
fun KTunePerfTopScreen() {
    val ksuCli = koinInject<KsuCliRepository>()
    var lines by remember { mutableStateOf<List<String>>(emptyList()) }

    LaunchedEffect(Unit) {
        while (true) {
            lines = withContext(Dispatchers.IO) { ksuCli.getPerfTopProcesses() }
            delay(2000L)
        }
    }

    KTuneSubScaffold(title = "PerfTop") {
        items(lines) { line ->
            Text(
                text = line,
                style = MaterialTheme.typography.bodySmall,
                fontFamily = FontFamily.Monospace,
                modifier = Modifier.padding(vertical = 2.dp),
            )
        }
    }
}
