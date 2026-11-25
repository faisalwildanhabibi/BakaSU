package org.bakasu.bakasu.data.system

import android.annotation.SuppressLint
import android.app.ActivityManager
import android.app.Application
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.BatteryManager
import android.os.Build
import android.system.Os
import java.util.Locale
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.bakasu.bakasu.BuildConfig
import org.bakasu.bakasu.domain.model.HomeBasicInfo

class HomeRuntimeRepository(
    private val application: Application,
) {
    suspend fun getBasicInfo(
        managerUapiVersion: Int,
        includeSelinuxStatus: Boolean = true,
    ): HomeBasicInfo = withContext(Dispatchers.IO) {
        val uname = runCatching { Os.uname() }.getOrNull()
        val memInfo = getMemoryInfo()
        val battery = getBatteryInfo()
        HomeBasicInfo(
            kernelRelease = uname?.release ?: "Unknown",
            androidVersion = Build.VERSION.RELEASE ?: "Unknown",
            deviceModel = getDeviceModel(),
            managerVersion = Triple(
                BuildConfig.VERSION_NAME,
                BuildConfig.VERSION_CODE,
                managerUapiVersion,
            ),
            selinuxStatus = if (includeSelinuxStatus) {
                runCatching { getSELinuxStatus(application) }.getOrDefault("Unknown")
            } else {
                ""
            },
            seccompStatus = runCatching { Os.prctl(21, 0, 0, 0, 0) }.getOrDefault(-1),
            socModel = getSocModel(),
            cpuCores = Runtime.getRuntime().availableProcessors(),
            totalMemory = memInfo.first,
            availMemory = memInfo.second,
            batteryLevel = battery.first,
            batteryTemp = battery.second,
            batteryStatus = battery.third,
            securityPatch = Build.VERSION.SECURITY_PATCH.orEmpty(),
        )
    }

    @SuppressLint("PrivateApi")
    private fun getSocModel(): String = runCatching {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S && Build.SOC_MODEL.isNotBlank()) {
            Build.SOC_MODEL
        } else {
            val systemProperties = Class.forName("android.os.SystemProperties")
            val getMethod = systemProperties.getMethod("get", String::class.java, String::class.java)
            val socKeys = listOf(
                "ro.soc.model",
                "ro.board.platform",
                "ro.hardware",
            )
            socKeys.firstNotNullOfOrNull { key ->
                (getMethod.invoke(null, key, "") as? String)?.takeIf(String::isNotEmpty)
            } ?: Build.HARDWARE
        }
    }.getOrDefault(Build.HARDWARE)

    private fun getMemoryInfo(): Pair<String, String> = runCatching {
        val actManager = application.getSystemService(Context.ACTIVITY_SERVICE) as? ActivityManager
        val memInfo = ActivityManager.MemoryInfo()
        actManager?.getMemoryInfo(memInfo)
        val totalGb = String.format(Locale.US, "%.1f GB", memInfo.totalMem.toDouble() / (1024 * 1024 * 1024))
        val availGb = String.format(Locale.US, "%.1f GB", memInfo.availMem.toDouble() / (1024 * 1024 * 1024))
        Pair(totalGb, availGb)
    }.getOrDefault(Pair("Unknown", "Unknown"))

    private fun getBatteryInfo(): Triple<Int, Float, String> = runCatching {
        val ifilter = IntentFilter(Intent.ACTION_BATTERY_CHANGED)
        val bStatus = application.registerReceiver(null, ifilter)
        val level = bStatus?.getIntExtra(BatteryManager.EXTRA_LEVEL, -1) ?: -1
        val scale = bStatus?.getIntExtra(BatteryManager.EXTRA_SCALE, -1) ?: -1
        val pct = if (level >= 0 && scale > 0) (level * 100) / scale else -1
        val tempTenths = bStatus?.getIntExtra(BatteryManager.EXTRA_TEMPERATURE, 0) ?: 0
        val tempC = tempTenths / 10f
        val status = when (bStatus?.getIntExtra(BatteryManager.EXTRA_STATUS, -1)) {
            BatteryManager.BATTERY_STATUS_CHARGING -> "Charging"
            BatteryManager.BATTERY_STATUS_DISCHARGING -> "Discharging"
            BatteryManager.BATTERY_STATUS_FULL -> "Full"
            BatteryManager.BATTERY_STATUS_NOT_CHARGING -> "Not charging (Bypass)"
            else -> "Unknown"
        }
        Triple(pct, tempC, status)
    }.getOrDefault(Triple(-1, 0f, "Unknown"))

    @SuppressLint("PrivateApi")
    private fun getDeviceModel(): String = runCatching {
        val systemProperties = Class.forName("android.os.SystemProperties")
        val getMethod = systemProperties.getMethod("get", String::class.java, String::class.java)
        val marketNameKeys = listOf(
            "ro.product.marketname",
            "ro.vendor.oplus.market.name",
            "ro.vivo.market.name",
            "ro.config.marketing_name",
        )
        marketNameKeys.firstNotNullOfOrNull { key ->
            (getMethod.invoke(null, key, "") as? String)?.takeIf(String::isNotEmpty)
        } ?: getDeviceInfo()
    }.getOrDefault(getDeviceInfo())

    private fun getDeviceInfo(): String = runCatching {
        val manufacturer = Build.MANUFACTURER.orEmpty().replaceFirstChar { it.uppercase() }
        val brand = Build.BRAND.orEmpty().takeUnless {
            it.equals(Build.MANUFACTURER, ignoreCase = true)
        }.orEmpty()
        listOf(manufacturer, brand, Build.MODEL.orEmpty())
            .filter(String::isNotBlank)
            .joinToString(" ")
    }.getOrDefault("Unknown Device")
}
