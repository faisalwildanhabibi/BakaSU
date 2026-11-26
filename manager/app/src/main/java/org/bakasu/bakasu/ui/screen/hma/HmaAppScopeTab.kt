package org.bakasu.bakasu.ui.screen.hma

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.twotone.Search
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
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

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun HmaAppScopeTab(
    config: KsuCliRepository.HmaConfig,
    installedPackages: List<InstalledPackageInfo>,
    contentPadding: PaddingValues,
    onUpdateScope: (enabled: Boolean, scope: KsuCliRepository.HmaAppScope) -> Unit,
) {
    var query by remember { mutableStateOf("") }
    var showUserApps by remember { mutableStateOf(true) }
    var showSystemApps by remember { mutableStateOf(false) }
    var onlyHidden by remember { mutableStateOf(false) }
    var onlyAffected by remember { mutableStateOf(false) }
    var editingPkg by remember { mutableStateOf<InstalledPackageInfo?>(null) }

    val hiddenPkgSet = remember(config.templates) {
        config.templates.values.flatMap { it.appList }.toSet()
    }

    val filteredApps = remember(
        installedPackages,
        config.scope,
        hiddenPkgSet,
        query,
        showUserApps,
        showSystemApps,
        onlyHidden,
        onlyAffected,
    ) {
        val q = query.trim().lowercase()
        installedPackages.filter { pkg ->
            val inScope = config.scope.containsKey(pkg.packageName)
            val isHidden = hiddenPkgSet.contains(pkg.packageName)
            val categoryMatch = (showUserApps && !pkg.isSystem) || (showSystemApps && pkg.isSystem)
            val hiddenMatch = !onlyHidden || isHidden
            val affectedMatch = !onlyAffected || inScope
            val queryMatch = q.isEmpty() ||
                pkg.appLabel.lowercase().contains(q) ||
                pkg.packageName.lowercase().contains(q)
            categoryMatch && hiddenMatch && affectedMatch && queryMatch
        }.sortedWith(
            compareByDescending<InstalledPackageInfo> { config.scope.containsKey(it.packageName) }
                .thenBy { it.appLabel.lowercase() },
        )
    }

    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = contentPadding,
    ) {
        item {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 8.dp),
            ) {
                OutlinedTextField(
                    value = query,
                    onValueChange = { query = it },
                    placeholder = { Text("Search target apps...") },
                    leadingIcon = { Icon(Icons.TwoTone.Search, contentDescription = null) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                Spacer(Modifier.height(8.dp))
                FlowRow(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    FilterChip(
                        selected = showUserApps,
                        onClick = { showUserApps = !showUserApps },
                        label = { Text("Apps") },
                    )
                    FilterChip(
                        selected = showSystemApps,
                        onClick = { showSystemApps = !showSystemApps },
                        label = { Text("System") },
                    )
                    FilterChip(
                        selected = onlyHidden,
                        onClick = { onlyHidden = !onlyHidden },
                        label = { Text("Hidden (${hiddenPkgSet.size})") },
                    )
                    FilterChip(
                        selected = onlyAffected,
                        onClick = { onlyAffected = !onlyAffected },
                        label = { Text("Affected (${config.scope.size})") },
                    )
                }
            }
        }

        items(filteredApps, key = { it.packageName }) { pkg ->
            val appScope = config.scope[pkg.packageName]
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { editingPkg = pkg }
                    .padding(horizontal = 16.dp, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                PackageIcon(
                    packageName = pkg.packageName,
                    contentDescription = pkg.appLabel,
                    modifier = Modifier.size(40.dp),
                )
                Spacer(Modifier.width(12.dp))
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = pkg.appLabel.ifEmpty { pkg.packageName },
                        style = MaterialTheme.typography.titleSmall,
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
                Spacer(Modifier.width(8.dp))
                Surface(
                    shape = MaterialTheme.shapes.small,
                    color = if (appScope != null) {
                        MaterialTheme.colorScheme.primaryContainer
                    } else {
                        MaterialTheme.colorScheme.surfaceVariant
                    },
                ) {
                    val badgeText = if (appScope == null) {
                        "Off"
                    } else {
                        val mode = if (appScope.useWhitelist) "Whitelist" else "Blacklist"
                        "$mode • ${appScope.applyTemplates.size}T/${appScope.extraAppList.size}E"
                    }
                    Text(
                        text = badgeText,
                        style = MaterialTheme.typography.labelSmall,
                        color = if (appScope != null) {
                            MaterialTheme.colorScheme.onPrimaryContainer
                        } else {
                            MaterialTheme.colorScheme.onSurfaceVariant
                        },
                        modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp),
                    )
                }
            }
        }
    }

    editingPkg?.let { target ->
        HmaScopeEditorDialog(
            pkg = target,
            initialScope = config.scope[target.packageName],
            templates = config.templates,
            installedPackages = installedPackages,
            onDismiss = { editingPkg = null },
            onSave = { enabled, updated ->
                editingPkg = null
                onUpdateScope(enabled, updated)
            },
        )
    }
}
