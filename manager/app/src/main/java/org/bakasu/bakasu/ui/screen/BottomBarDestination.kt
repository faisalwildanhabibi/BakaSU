package org.bakasu.bakasu.ui.screen

import androidx.annotation.StringRes
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.twotone.AdminPanelSettings
import androidx.compose.material.icons.twotone.Extension
import androidx.compose.material.icons.twotone.Home
import androidx.compose.material.icons.twotone.Security
import androidx.compose.material.icons.twotone.Settings
import androidx.compose.material.icons.twotone.Tune
import androidx.compose.material.icons.twotone.VisibilityOff
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.Dp
import org.bakasu.bakasu.R
import org.bakasu.bakasu.ui.screen.hma.HMAScreen
import org.bakasu.bakasu.ui.screen.ktune.KTuneScreen
import org.bakasu.bakasu.ui.screen.main.HomePage
import org.bakasu.bakasu.ui.screen.main.ModulePage
import org.bakasu.bakasu.ui.screen.main.SettingsPage
import org.bakasu.bakasu.ui.screen.main.SuperUserPage
import org.bakasu.bakasu.ui.screen.zygisk.ZygiskScreen

enum class BottomBarDestination(
    val direction: @Composable (bottomPadding: Dp) -> Unit,
    @param:StringRes val label: Int,
    val iconSelected: ImageVector,
    val iconNotSelected: ImageVector,
    val rootRequired: Boolean,
) {
    Home(
        { bottomPadding -> HomePage(bottomPadding) },
        R.string.home,
        Icons.TwoTone.Home,
        Icons.TwoTone.Home,
        false,
    ),
    SuperUser(
        { bottomPadding -> SuperUserPage(bottomPadding) },
        R.string.superuser,
        Icons.TwoTone.AdminPanelSettings,
        Icons.TwoTone.AdminPanelSettings,
        true,
    ),
    Module(
        { bottomPadding -> ModulePage(bottomPadding) },
        R.string.module,
        Icons.TwoTone.Extension,
        Icons.TwoTone.Extension,
        true,
    ),
    KTune(
        { bottomPadding -> KTuneScreen(showBackButton = false, bottomPadding = bottomPadding) },
        R.string.nav_ktune,
        Icons.TwoTone.Tune,
        Icons.TwoTone.Tune,
        true,
    ),
    Zygisk(
        { bottomPadding -> ZygiskScreen(showBackButton = false, bottomPadding = bottomPadding) },
        R.string.nav_zygisk,
        Icons.TwoTone.Security,
        Icons.TwoTone.Security,
        true,
    ),
    HMA(
        { bottomPadding -> HMAScreen(showBackButton = false, bottomPadding = bottomPadding) },
        R.string.nav_hma,
        Icons.TwoTone.VisibilityOff,
        Icons.TwoTone.VisibilityOff,
        true,
    ),
    Settings(
        { bottomPadding -> SettingsPage(bottomPadding) },
        R.string.settings,
        Icons.TwoTone.Settings,
        Icons.TwoTone.Settings,
        false,
    ),
    ;

    companion object {
        fun getPages(isKsuValid: Boolean): List<BottomBarDestination> = if (isKsuValid) {
            // 全功能管理器
            BottomBarDestination.entries.toList()
        } else {
            BottomBarDestination.entries.filter {
                !it.rootRequired
            }
        }
    }
}
