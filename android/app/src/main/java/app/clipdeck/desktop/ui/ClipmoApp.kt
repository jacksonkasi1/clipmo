@file:OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)

package app.clipdeck.desktop.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animate
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.ExperimentalMaterialApi
import androidx.compose.material.pullrefresh.PullRefreshIndicator
import androidx.compose.material.pullrefresh.pullRefresh
import androidx.compose.material.pullrefresh.rememberPullRefreshState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.ime
import kotlin.math.roundToInt
import app.clipdeck.desktop.data.ClipKind
import app.clipdeck.desktop.data.ClipRecord
import app.clipdeck.desktop.data.TrustedDeviceRecord
import app.clipdeck.desktop.R
import app.clipdeck.desktop.ui.theme.ClipmoTheme
import app.clipdeck.desktop.ui.theme.ClipmoThemeMode
import java.text.DateFormat
import java.util.Date
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private enum class ClipmoScreen { HISTORY, COLLECTIONS, DEVICES, SETTINGS }
private enum class ClipFilter(val label: String) { ALL("All"), TEXT("Text"), LINKS("Links"), IMAGES("Images"), FILES("Files"), STARRED("Starred") }

/**
 * Decoded thumbnail cache shared across history, collections, and re-scrolls.
 * Bitmaps are only ever touched from the main thread after being produced on
 * Dispatchers.IO, so the plain LruCache needs no synchronization.
 */
private object ClipmoThumbCache {
    private val cache = object : android.util.LruCache<String, android.graphics.Bitmap>(16 * 1024 * 1024) {
        override fun sizeOf(key: String, value: android.graphics.Bitmap): Int = value.allocationByteCount
    }

    operator fun get(key: String): android.graphics.Bitmap? = cache.get(key)

    operator fun set(key: String, value: android.graphics.Bitmap) { cache.put(key, value) }
}

private object ClipmoTimeFormat {
    val short: DateFormat = DateFormat.getTimeInstance(DateFormat.SHORT)
}

data class ClipmoUiState(
    val clips: List<ClipRecord>,
    val monitorEnabled: Boolean,
    val screenshotCaptureEnabled: Boolean,
    val syncEnabled: Boolean,
    val copyLiveSyncToClipboard: Boolean,
    val pairingCode: String,
    val pairingModeActive: Boolean,
    val localDeviceName: String,
    val localDeviceId: String,
    val themeMode: ClipmoThemeMode,
    val trustedDevices: List<TrustedDeviceRecord>,
    val collections: List<String>,
)

@Composable
fun ClipmoApp(
    state: ClipmoUiState,
    onCopy: (ClipRecord) -> Unit,
    onFavorite: (ClipRecord) -> Unit,
    onDelete: (ClipRecord) -> Unit,
    onFavoriteMany: (Set<Long>) -> Unit,
    onDeleteMany: (Set<Long>) -> Unit,
    onCreateCollection: (String) -> Unit,
    onAddToCollection: (Set<Long>, String) -> Unit,
    onEditClip: (id: Long, content: String) -> Unit,
    onClear: () -> Unit,
    onMonitorChanged: (Boolean) -> Unit,
    onScreenshotCaptureChanged: (Boolean) -> Unit,
    onSyncChanged: (Boolean) -> Unit,
    onCopyLiveSyncChanged: (Boolean) -> Unit,
    onPairingCodeChanged: (String) -> Unit,
    onThemeChanged: (ClipmoThemeMode) -> Unit,
    onForgetDevice: (TrustedDeviceRecord) -> Unit,
    onStartPairing: () -> Unit,
    onRefresh: () -> Unit,
) {
    ClipmoTheme(state.themeMode) {
        var screen by rememberSaveable { mutableStateOf(ClipmoScreen.HISTORY) }
        var pendingDelete by remember { mutableStateOf<ClipRecord?>(null) }
        var pendingClear by remember { mutableStateOf(false) }
        // Copy is the app's peak action; every tab funnels through this wrapper
        // so one animated confirmation pill covers cards, sheets, and dialogs.
        var copyPulse by remember { mutableStateOf(0) }
        val copyWithFeedback: (ClipRecord) -> Unit = remember(onCopy) {
            { clip ->
                onCopy(clip)
                copyPulse++
            }
        }
        val colors = ClipmoTheme.colors
        Box(
            Modifier
                .fillMaxSize()
                .windowInsetsPadding(WindowInsets.safeDrawing)
                .background(colors.background),
        ) {
            Column(Modifier.fillMaxSize()) {
                ClipmoTopBar(
                    screen = screen,
                    onSettings = { screen = ClipmoScreen.SETTINGS },
                    onBack = if (screen == ClipmoScreen.SETTINGS) ({ screen = ClipmoScreen.HISTORY }) else null,
                )
                Box(Modifier.weight(1f)) {
                    when (screen) {
                        ClipmoScreen.HISTORY -> HistoryScreen(state, copyWithFeedback, onFavorite, onDelete, onFavoriteMany, onDeleteMany, onAddToCollection, onEditClip, onRefresh)
                        ClipmoScreen.COLLECTIONS -> CollectionsScreen(state, copyWithFeedback, onFavorite, { pendingDelete = it }, onCreateCollection)
                        ClipmoScreen.DEVICES -> DevicesScreen(state, onForgetDevice, onPairingCodeChanged, onSyncChanged, onStartPairing, onRefresh)
                        ClipmoScreen.SETTINGS -> SettingsScreen(
                            state = state,
                            onMonitorChanged = onMonitorChanged,
                            onScreenshotCaptureChanged = onScreenshotCaptureChanged,
                            onSyncChanged = onSyncChanged,
                            onCopyLiveSyncChanged = onCopyLiveSyncChanged,
                            onPairingCodeChanged = onPairingCodeChanged,
                            onThemeChanged = onThemeChanged,
                            onClear = { pendingClear = true },
                        )
                    }
                }
                if (screen != ClipmoScreen.SETTINGS) {
                    ClipmoBottomBar(screen) { screen = it }
                }
            }
            pendingDelete?.let { clip ->
                ClipmoConfirmOverlay(
                    title = "Delete clip?",
                    message = clip.content.take(100),
                    confirmLabel = "Delete",
                    onDismiss = { pendingDelete = null },
                    onConfirm = { onDelete(clip); pendingDelete = null },
                )
            }
            if (pendingClear) {
                ClipmoConfirmOverlay(
                    title = "Clear clipboard history?",
                    message = "This removes every local clip and cannot be undone.",
                    confirmLabel = "Clear all",
                    onDismiss = { pendingClear = false },
                    onConfirm = { onClear(); pendingClear = false },
                )
            }
            ClipmoCopiedPill(copyPulse, Modifier.align(Alignment.BottomCenter))
        }
    }
}

@Composable
private fun ClipmoTopBar(screen: ClipmoScreen, onSettings: () -> Unit, onBack: (() -> Unit)?) {
    val colors = ClipmoTheme.colors
    val type = ClipmoTheme.typography
    val space = ClipmoTheme.spacing
    Row(
        Modifier
            .fillMaxWidth()
            .height(ClipmoTheme.dimensions.topBarHeight)
            .padding(horizontal = space.md),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (onBack != null) {
            ClipmoIconButton(ClipmoIconKind.BACK, "Back", onBack)
            Spacer(Modifier.width(space.xs))
        } else {
            Image(
                painter = painterResource(R.drawable.clipmo_logo),
                contentDescription = "Clipmo logo",
                modifier = Modifier.size(26.dp).clip(RoundedCornerShape(7.dp)),
            )
            Spacer(Modifier.width(space.sm))
        }
        androidx.compose.foundation.text.BasicText(
            text = if (screen == ClipmoScreen.SETTINGS) "Settings" else "Clipmo",
            style = if (screen == ClipmoScreen.SETTINGS) type.title.copy(color = colors.textPrimary) else type.brand.copy(color = colors.textPrimary),
            modifier = Modifier.weight(1f),
        )
        if (screen != ClipmoScreen.SETTINGS) ClipmoIconButton(ClipmoIconKind.SETTINGS, "Settings", onSettings)
    }
}

@OptIn(ExperimentalMaterialApi::class)
@Composable
private fun HistoryScreen(
    state: ClipmoUiState,
    onCopy: (ClipRecord) -> Unit,
    onFavorite: (ClipRecord) -> Unit,
    onDelete: (ClipRecord) -> Unit,
    onFavoriteMany: (Set<Long>) -> Unit,
    onDeleteMany: (Set<Long>) -> Unit,
    onAddToCollection: (Set<Long>, String) -> Unit,
    onEditClip: (id: Long, content: String) -> Unit,
    onRefresh: () -> Unit,
) {
    val clips = state.clips
    var query by rememberSaveable { mutableStateOf("") }
    var filter by rememberSaveable { mutableStateOf(ClipFilter.ALL) }
    var selectedDevice by rememberSaveable { mutableStateOf("all") }
    var refreshing by remember { mutableStateOf(false) }
    var selectedIds by remember { mutableStateOf<Set<Long>>(emptySet()) }
    var pendingBulkDelete by remember { mutableStateOf<Set<Long>?>(null) }
    var collectionPickerIds by remember { mutableStateOf<Set<Long>?>(null) }
    var detailClip by remember { mutableStateOf<ClipRecord?>(null) }
    // lastDetail keeps the clip composed while the sheet slides out after
    // detailClip is cleared; sheetOpen drives both slide/fade animations.
    var sheetOpen by remember { mutableStateOf(false) }
    var lastDetail by remember { mutableStateOf<ClipRecord?>(null) }
    LaunchedEffect(detailClip) {
        detailClip?.let { lastDetail = it }
        sheetOpen = detailClip != null
    }
    val devices = remember(clips, state.trustedDevices, state.localDeviceId) {
        listOf(Triple("all", "All devices", clips.size), Triple(state.localDeviceId, "This phone", clips.count { it.isLocalTo(state.localDeviceId) })) +
            state.trustedDevices.map { device -> Triple(device.id, device.name, clips.count { it.originDevice == device.id }) }
    }
    LaunchedEffect(devices.map { it.first }) {
        if (devices.none { it.first == selectedDevice }) selectedDevice = "all"
    }
    LaunchedEffect(clips.map(ClipRecord::id)) {
        selectedIds = selectedIds.intersect(clips.mapTo(mutableSetOf(), ClipRecord::id))
    }
    LaunchedEffect(refreshing) {
        if (refreshing) {
            onRefresh()
            delay(700)
            refreshing = false
        }
    }
    val shown = remember(clips, query, filter, selectedDevice) {
        clips.filter { clip ->
            (selectedDevice == "all" || if (selectedDevice == state.localDeviceId) clip.isLocalTo(state.localDeviceId) else clip.originDevice == selectedDevice) &&
                clip.content.contains(query, ignoreCase = true) && when (filter) {
                ClipFilter.ALL -> true
                ClipFilter.TEXT -> clip.kind == ClipKind.TEXT
                ClipFilter.LINKS -> clip.kind == ClipKind.URL
                ClipFilter.IMAGES -> clip.kind == ClipKind.IMAGE
                ClipFilter.FILES -> clip.kind == ClipKind.FILE
                ClipFilter.STARRED -> clip.favorite
            }
        }
    }
    // Grouping ~2k clips ran on every recomposition (search keystrokes,
    // selection changes); memoizing keeps it to actual data changes.
    val grouped = remember(shown, state.trustedDevices, state.localDeviceId) {
        val trustedNames = state.trustedDevices.associate { it.id to it.name }
        shown.groupBy { clip ->
            when {
                clip.isLocalTo(state.localDeviceId) -> "This Android phone"
                !clip.originDevice.isNullOrBlank() -> trustedNames[clip.originDevice] ?: sourceLabel(clip.source)
                else -> sourceLabel(clip.source)
            }
        }
    }
    val pullState = rememberPullRefreshState(refreshing, { refreshing = true })
    Box(Modifier.fillMaxSize().pullRefresh(pullState)) {
      LazyColumn(modifier = Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = ClipmoTheme.spacing.lg)) {
        item {
            Column(Modifier.padding(horizontal = ClipmoTheme.spacing.md)) {
                if (selectedIds.isNotEmpty()) {
                    ClipmoSelectionBar(
                        selectedCount = selectedIds.size,
                        onStar = {
                            onFavoriteMany(selectedIds)
                            selectedIds = emptySet()
                        },
                        onDelete = { pendingBulkDelete = selectedIds },
                        onCollection = { collectionPickerIds = selectedIds },
                        onClose = { selectedIds = emptySet() },
                    )
                } else {
                    ClipmoSearchBar(value = query, hint = "Search clips", onValueChange = { query = it })
                }
                Spacer(Modifier.height(ClipmoTheme.spacing.sm))
                Row(
                    Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),
                    horizontalArrangement = Arrangement.spacedBy(ClipmoTheme.spacing.xs),
                ) {
                    devices.forEach { (id, name, count) ->
                        ClipmoPill("$name · $count", selectedDevice == id) { selectedDevice = id }
                    }
                }
                Spacer(Modifier.height(ClipmoTheme.spacing.sm))
                ClipmoFilterRow(filter) { filter = it }
                Spacer(Modifier.height(ClipmoTheme.spacing.lg))
            }
        }
        if (shown.isEmpty()) {
            item { ClipmoEmptyState(if (clips.isEmpty()) "Your clipboard is quiet" else "No matching clips", "Copy something on this device to see it here.") }
        } else {
            grouped.forEach { (device, deviceClips) ->
                item { ClipmoDeviceHeader(device, deviceClips.size) }
                items(deviceClips, key = { it.id }, contentType = { "clip" }) { clip ->
                    ClipmoSwipeToDelete(
                        clip = clip,
                        enabled = selectedIds.isEmpty(),
                        onDelete = onDelete,
                        onChooseCollection = { collectionPickerIds = setOf(clip.id) },
                    ) {
                        ClipmoClipboardCard(
                            clip = clip,
                            onCopy = onCopy,
                            onFavorite = onFavorite,
                            onDelete = onDelete,
                            selected = clip.id in selectedIds,
                            selectionMode = selectedIds.isNotEmpty(),
                            onSelect = {
                                selectedIds = if (clip.id in selectedIds) selectedIds - clip.id else selectedIds + clip.id
                            },
                            onOpen = { detailClip = clip },
                        )
                    }
                }
                item { Spacer(Modifier.height(ClipmoTheme.spacing.md)) }
            }
        }
      }
      PullRefreshIndicator(refreshing, pullState, Modifier.align(Alignment.TopCenter), contentColor = ClipmoTheme.colors.accent)
      pendingBulkDelete?.let { ids ->
          ClipmoConfirmOverlay(
              title = "Delete ${ids.size} clips?",
              message = "The selected clips will be removed from synced devices.",
              confirmLabel = "Delete",
              onDismiss = { pendingBulkDelete = null },
              onConfirm = {
                  onDeleteMany(ids)
                  selectedIds = emptySet()
                  pendingBulkDelete = null
              },
          )
      }
      collectionPickerIds?.let { ids ->
          ClipmoCollectionPickerOverlay(
              collections = state.collections,
              selectedCount = ids.size,
              onDismiss = { collectionPickerIds = null },
              onCollection = { collection ->
                  onAddToCollection(ids, collection)
                  selectedIds = emptySet()
                  collectionPickerIds = null
              },
          )
      }
      // Scrim fades while the sheet itself slides; lastDetail keeps content
      // alive for the exit animation after detailClip is cleared.
      AnimatedVisibility(sheetOpen, enter = fadeIn(tween(160)), exit = fadeOut(tween(160))) {
          Box(
              Modifier
                  .fillMaxSize()
                  .background(ClipmoTheme.colors.scrim)
                  .combinedClickable(onClick = { detailClip = null }),
          )
      }
      AnimatedVisibility(
          sheetOpen,
          enter = slideInVertically(tween(240)) { it },
          exit = slideOutVertically(tween(200)) { it },
      ) {
          lastDetail?.let { clip ->
              ClipmoDetailSheet(
                  clip = clip,
                  onCopy = onCopy,
                  onDelete = {
                      onDelete(clip)
                      detailClip = null
                  },
                  onEdit = { content ->
                      onEditClip(clip.id, content)
                      detailClip = null
                  },
                  onDismiss = { detailClip = null },
              )
          }
      }
    }
}

@Composable
private fun CollectionsScreen(
    state: ClipmoUiState,
    onCopy: (ClipRecord) -> Unit,
    onFavorite: (ClipRecord) -> Unit,
    onDelete: (ClipRecord) -> Unit,
    onCreateCollection: (String) -> Unit,
) {
    val clips = state.clips
    val tags = remember(clips, state.collections) {
        (state.collections + clips.flatMap(ClipRecord::tags)).distinctBy(String::lowercase).sortedBy(String::lowercase)
    }
    var selected by rememberSaveable { mutableStateOf<String?>(null) }
    var creatingCollection by remember { mutableStateOf(false) }
    LaunchedEffect(tags) { if (selected !in tags) selected = tags.firstOrNull() }
    val selectedClips = clips.filter { selected != null && selected in it.tags }
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = ClipmoTheme.spacing.lg)) {
        item {
            Column(Modifier.padding(horizontal = ClipmoTheme.spacing.md)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    androidx.compose.foundation.text.BasicText("Tag collections", style = ClipmoTheme.typography.section.copy(color = ClipmoTheme.colors.textSecondary), modifier = Modifier.weight(1f))
                    ClipmoPill("+ New", false) { creatingCollection = true }
                }
                Spacer(Modifier.height(ClipmoTheme.spacing.sm))
                if (tags.isEmpty()) ClipmoEmptyState("No tag collections yet", "Create a collection, then long-press clips to add them.")
                else Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(ClipmoTheme.spacing.sm)) {
                    tags.forEach { tag ->
                        ClipmoCollectionCard(tag, ClipmoIconKind.COLLECTION, clips.count { tag in it.tags }, selected == tag, Modifier.width(138.dp)) { selected = tag }
                    }
                }
                Spacer(Modifier.height(ClipmoTheme.spacing.xl))
                selected?.let { androidx.compose.foundation.text.BasicText(it, style = ClipmoTheme.typography.section.copy(color = ClipmoTheme.colors.textPrimary)) }
                Spacer(Modifier.height(ClipmoTheme.spacing.sm))
            }
        }
        if (tags.isNotEmpty() && selectedClips.isEmpty()) item { ClipmoEmptyState("No clips here yet", "Tagged clips will appear in this collection.") }
        items(selectedClips, key = { it.id }) { ClipmoClipboardCard(it, onCopy, onFavorite, onDelete) }
    }
    if (creatingCollection) {
        ClipmoCreateCollectionOverlay(
            onDismiss = { creatingCollection = false },
            onCreate = {
                onCreateCollection(it)
                selected = it.trim()
                creatingCollection = false
            },
        )
    }
}

@Composable
private fun DevicesScreen(
    state: ClipmoUiState,
    onForgetDevice: (TrustedDeviceRecord) -> Unit,
    onPairingCodeChanged: (String) -> Unit,
    onSyncChanged: (Boolean) -> Unit,
    onStartPairing: () -> Unit,
    onRefresh: () -> Unit,
) {
    val colors = ClipmoTheme.colors
    val space = ClipmoTheme.spacing
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(horizontal = space.md, vertical = space.sm)) {
        item {
            androidx.compose.foundation.text.BasicText("Add a device", style = ClipmoTheme.typography.section.copy(color = colors.textSecondary))
            Spacer(Modifier.height(space.sm))
            Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(ClipmoTheme.shapes.panel)).background(colors.surface).border(1.dp, colors.border, RoundedCornerShape(ClipmoTheme.shapes.panel)).padding(space.md)) {
                androidx.compose.foundation.text.BasicText("Use the same pairing code on every Clipmo device.", style = ClipmoTheme.typography.body.copy(color = colors.textPrimary))
                Spacer(Modifier.height(space.sm))
                ClipmoSearchBar(state.pairingCode, "Pairing code", onPairingCodeChanged, searchIcon = false)
                Spacer(Modifier.height(space.sm))
                Row(horizontalArrangement = Arrangement.spacedBy(space.sm)) {
                    ClipmoButton(
                        if (state.pairingModeActive) "Pairing…" else "Add another device",
                        if (state.pairingModeActive) ClipmoButtonStyle.SECONDARY else ClipmoButtonStyle.PRIMARY,
                        Modifier.weight(1f),
                        icon = ClipmoIconKind.PLUS,
                    ) {
                        if (!state.syncEnabled) onSyncChanged(true)
                        onStartPairing()
                    }
                    ClipmoButton("Refresh", ClipmoButtonStyle.GHOST, Modifier.weight(1f), icon = ClipmoIconKind.REFRESH, onClick = onRefresh)
                }
            }
            Spacer(Modifier.height(space.xl))
            androidx.compose.foundation.text.BasicText("This device", style = ClipmoTheme.typography.section.copy(color = colors.textSecondary))
            Spacer(Modifier.height(space.sm))
            ClipmoDeviceCard(state.localDeviceName, "This phone", "Ready", true)
            Spacer(Modifier.height(space.xl))
            androidx.compose.foundation.text.BasicText("Paired devices", style = ClipmoTheme.typography.section.copy(color = colors.textSecondary))
            Spacer(Modifier.height(space.sm))
            if (state.trustedDevices.isEmpty()) {
                ClipmoEmptyState(
                    if (state.syncEnabled) "Looking for your devices" else "Sync is off",
                    if (state.syncEnabled) "Paired devices will appear independently as they reconnect on your LAN." else "Enable sync in Settings to discover and pair multiple devices.",
                )
            } else {
                // Single full-width rows: two-up grid cramped real device names
                // against the Forget action and overflowed on narrow screens.
                state.trustedDevices.forEach { device ->
                    ClipmoDeviceCard(
                        name = device.name,
                        platformLabel = device.platform.replaceFirstChar(Char::uppercase),
                        statusLabel = if (device.online) "Connected" else "Seen ${relativeSeen(device.lastSeenMs)}",
                        online = device.online,
                        action = "Forget",
                        onAction = { onForgetDevice(device) },
                    )
                    Spacer(Modifier.height(space.sm))
                }
            }
        }
    }
}

@Composable
private fun SettingsScreen(
    state: ClipmoUiState,
    onMonitorChanged: (Boolean) -> Unit,
    onScreenshotCaptureChanged: (Boolean) -> Unit,
    onSyncChanged: (Boolean) -> Unit,
    onCopyLiveSyncChanged: (Boolean) -> Unit,
    onPairingCodeChanged: (String) -> Unit,
    onThemeChanged: (ClipmoThemeMode) -> Unit,
    onClear: () -> Unit,
) {
    val colors = ClipmoTheme.colors
    val space = ClipmoTheme.spacing
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(horizontal = space.md, vertical = space.sm)) {
        item {
            ClipmoSettingsGroup("Clipboard") {
                ClipmoSettingsToggle("Clipboard monitoring", "Capture clipboard changes while Clipmo is allowed to run.", state.monitorEnabled, onMonitorChanged)
                Spacer(Modifier.height(space.md))
                ClipmoSettingsToggle("Screenshot capture", "Automatically save new screenshots to history while monitoring runs. Needs photo access only when enabled.", state.screenshotCaptureEnabled, onScreenshotCaptureChanged)
            }
            Spacer(Modifier.height(space.lg))
            ClipmoSettingsGroup("Local sync") {
                ClipmoSettingsToggle("LAN sync", "Reconnect to every trusted device on the same network.", state.syncEnabled, onSyncChanged)
                Spacer(Modifier.height(space.md))
                ClipmoSettingsToggle(
                    "Copy live synced clips",
                    "Off keeps incoming history inside Clipmo. On copies only newly received text to the phone clipboard, never reconnect history.",
                    state.copyLiveSyncToClipboard,
                    onCopyLiveSyncChanged,
                )
                Spacer(Modifier.height(space.md))
                androidx.compose.foundation.text.BasicText("Pairing code", style = ClipmoTheme.typography.label.copy(color = colors.textSecondary))
                Spacer(Modifier.height(space.xs))
                ClipmoSearchBar(state.pairingCode, "Enter code", onPairingCodeChanged, searchIcon = false)
            }
            Spacer(Modifier.height(space.lg))
            ClipmoSettingsGroup("Appearance") {
                Row(horizontalArrangement = Arrangement.spacedBy(space.sm)) {
                    ClipmoThemeMode.entries.forEach { mode ->
                        ClipmoPill(mode.name.lowercase().replaceFirstChar { it.uppercase() }, state.themeMode == mode, Modifier.weight(1f)) { onThemeChanged(mode) }
                    }
                }
            }
            Spacer(Modifier.height(space.lg))
            ClipmoSettingsGroup("History") {
                ClipmoActionRow("Clear clipboard history", "Remove every locally stored clip", colors.danger, onClear)
            }
        }
    }
}

@Composable
private fun ClipmoSearchBar(value: String, hint: String, onValueChange: (String) -> Unit, searchIcon: Boolean = true) {
    val colors = ClipmoTheme.colors
    val shape = RoundedCornerShape(ClipmoTheme.shapes.search)
    Row(
        Modifier
            .fillMaxWidth()
            .height(ClipmoTheme.dimensions.searchHeight)
            .clip(shape)
            .background(colors.surface)
            .border(1.dp, colors.border, shape)
            .padding(horizontal = ClipmoTheme.spacing.md),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (searchIcon) {
            ClipmoIcon(ClipmoIconKind.SEARCH, colors.textMuted, Modifier.size(14.dp))
            Spacer(Modifier.width(ClipmoTheme.spacing.sm))
        }
        BasicTextField(
            value = value,
            onValueChange = onValueChange,
            modifier = Modifier.weight(1f),
            textStyle = ClipmoTheme.typography.body.copy(color = colors.textPrimary),
            cursorBrush = SolidColor(colors.accent),
            singleLine = true,
            decorationBox = { inner ->
                Box(contentAlignment = Alignment.CenterStart) {
                    if (value.isEmpty()) androidx.compose.foundation.text.BasicText(hint, style = ClipmoTheme.typography.body.copy(color = colors.textMuted))
                    inner()
                }
            },
        )
    }
}

@Composable
private fun ClipmoFilterRow(selected: ClipFilter, onSelected: (ClipFilter) -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState()),
        horizontalArrangement = Arrangement.spacedBy(ClipmoTheme.spacing.xs),
    ) { ClipFilter.entries.forEach { filter -> ClipmoPill(filter.label, filter == selected) { onSelected(filter) } } }
}

@Composable
private fun ClipmoPill(label: String, selected: Boolean, modifier: Modifier = Modifier, onClick: () -> Unit) {
    val colors = ClipmoTheme.colors
    val background by animateColorAsState(if (selected) colors.accent else colors.surface, tween(150), label = "pill")
    val foreground by animateColorAsState(if (selected) colors.onAccent else colors.textSecondary, tween(150), label = "pillText")
    Box(
        modifier
            .height(ClipmoTheme.dimensions.chipHeight)
            .clip(RoundedCornerShape(ClipmoTheme.shapes.pill))
            .background(background)
            .border(1.dp, if (selected) Color.Transparent else colors.border, RoundedCornerShape(ClipmoTheme.shapes.pill))
            .combinedClickable(onClick = onClick)
            .padding(horizontal = ClipmoTheme.spacing.lg),
        contentAlignment = Alignment.Center,
    ) {
        androidx.compose.foundation.text.BasicText(
            label,
            style = ClipmoTheme.typography.label.copy(fontWeight = FontWeight.SemiBold, color = foreground),
        )
    }
}

private enum class ClipmoButtonStyle { PRIMARY, SECONDARY, DANGER, GHOST }

/** 48dp-tall action button used by sheets and dialogs; the accent variants
 *  carry the brand color, GHOST stays neutral (optionally danger-tinted). */
@Composable
private fun ClipmoButton(
    label: String,
    style: ClipmoButtonStyle,
    modifier: Modifier = Modifier,
    icon: ClipmoIconKind? = null,
    tint: Color? = null,
    onClick: () -> Unit,
) {
    val colors = ClipmoTheme.colors
    val background = when (style) {
        ClipmoButtonStyle.PRIMARY -> colors.accent
        ClipmoButtonStyle.SECONDARY -> colors.accentMuted
        ClipmoButtonStyle.DANGER -> colors.danger
        ClipmoButtonStyle.GHOST -> Color.Transparent
    }
    val content = tint ?: when (style) {
        ClipmoButtonStyle.PRIMARY -> colors.onAccent
        ClipmoButtonStyle.SECONDARY -> colors.accent
        ClipmoButtonStyle.DANGER -> Color.White
        ClipmoButtonStyle.GHOST -> colors.textSecondary
    }
    Row(
        modifier
            .height(ClipmoTheme.dimensions.actionHeight)
            .clip(RoundedCornerShape(ClipmoTheme.shapes.search))
            .background(background)
            .border(1.dp, if (style == ClipmoButtonStyle.GHOST) colors.border else Color.Transparent, RoundedCornerShape(ClipmoTheme.shapes.search))
            .combinedClickable(onClick = onClick)
            .padding(horizontal = ClipmoTheme.spacing.md),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) {
            ClipmoIcon(icon, content, Modifier.size(18.dp))
            Spacer(Modifier.width(ClipmoTheme.spacing.xs))
        }
        androidx.compose.foundation.text.BasicText(
            label,
            style = ClipmoTheme.typography.label.copy(fontWeight = FontWeight.SemiBold, color = content),
        )
    }
}

/** Floating confirmation shown after any copy; decorative only, taps pass through. */
@Composable
private fun ClipmoCopiedPill(copyPulse: Int, modifier: Modifier = Modifier) {
    val colors = ClipmoTheme.colors
    var visible by remember { mutableStateOf(false) }
    LaunchedEffect(copyPulse) {
        if (copyPulse > 0) {
            visible = true
            delay(1400)
            visible = false
        }
    }
    AnimatedVisibility(
        visible,
        modifier.padding(bottom = 80.dp),
        enter = fadeIn(tween(120)) + scaleIn(initialScale = 0.9f, animationSpec = tween(160)),
        exit = fadeOut(tween(200)) + scaleOut(targetScale = 0.95f, animationSpec = tween(160)),
    ) {
        Row(
            Modifier
                .clip(RoundedCornerShape(ClipmoTheme.shapes.pill))
                .background(colors.surfaceRaised)
                .border(1.dp, colors.border, RoundedCornerShape(ClipmoTheme.shapes.pill))
                .padding(horizontal = ClipmoTheme.spacing.lg, vertical = ClipmoTheme.spacing.sm),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            ClipmoIcon(ClipmoIconKind.CHECK, colors.accent, Modifier.size(16.dp))
            Spacer(Modifier.width(ClipmoTheme.spacing.xs))
            androidx.compose.foundation.text.BasicText(
                "Copied",
                style = ClipmoTheme.typography.label.copy(fontWeight = FontWeight.SemiBold, color = colors.textPrimary),
            )
        }
    }
}

@Composable
private fun ClipmoDeviceHeader(name: String, count: Int) {
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = ClipmoTheme.spacing.md, vertical = ClipmoTheme.spacing.xs),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        ClipmoIcon(ClipmoIconKind.DEVICE, ClipmoTheme.colors.textMuted, Modifier.size(13.dp))
        Spacer(Modifier.width(ClipmoTheme.spacing.xs))
        androidx.compose.foundation.text.BasicText(name, style = ClipmoTheme.typography.label.copy(color = ClipmoTheme.colors.textSecondary), modifier = Modifier.weight(1f))
        androidx.compose.foundation.text.BasicText(count.toString(), style = ClipmoTheme.typography.metadata.copy(color = ClipmoTheme.colors.textMuted))
    }
}

@Composable
private fun ClipmoSelectionBar(
    selectedCount: Int,
    onStar: () -> Unit,
    onDelete: () -> Unit,
    onCollection: () -> Unit,
    onClose: () -> Unit,
) {
    val colors = ClipmoTheme.colors
    Row(
        Modifier
            .fillMaxWidth()
            .height(ClipmoTheme.dimensions.searchHeight)
            .clip(RoundedCornerShape(ClipmoTheme.shapes.search))
            .background(colors.surface)
            .border(1.dp, colors.accent, RoundedCornerShape(ClipmoTheme.shapes.search))
            .padding(start = ClipmoTheme.spacing.md),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        androidx.compose.foundation.text.BasicText(
            "$selectedCount selected",
            style = ClipmoTheme.typography.body.copy(color = colors.textPrimary),
            modifier = Modifier.weight(1f),
        )
        ClipmoIconButton(ClipmoIconKind.COLLECTION, "Add selected clips to collection", onCollection, colors.accent)
        ClipmoIconButton(ClipmoIconKind.STAR, "Star selected clips", onStar, colors.accent)
        ClipmoIconButton(ClipmoIconKind.DELETE, "Delete selected clips", onDelete, colors.danger)
        ClipmoIconButton(ClipmoIconKind.BACK, "Exit selection", onClose)
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ClipmoSwipeToDelete(
    clip: ClipRecord,
    enabled: Boolean,
    onDelete: (ClipRecord) -> Unit,
    onChooseCollection: (ClipRecord) -> Unit,
    content: @Composable () -> Unit,
) {
    val colors = ClipmoTheme.colors
    var offsetX by remember { mutableStateOf(0f) }
    val scope = rememberCoroutineScope()
    val threshold = with(LocalDensity.current) { 96.dp.toPx() }

    // Material's SwipeToDismiss kept an anchored-draggable state machine and a
    // composed background alive on every row, which dominated fling cost in
    // ~2k-item history; this plain drag keeps the same gestures for a fraction
    // of the per-row work. The affordance composes only while dragged.
    Box(Modifier.fillMaxWidth()) {
        if (offsetX != 0f) {
            val movingToCollection = offsetX < 0
            Box(
                Modifier
                    .matchParentSize()
                    .padding(horizontal = ClipmoTheme.spacing.md, vertical = ClipmoTheme.spacing.xxs)
                    .clip(RoundedCornerShape(ClipmoTheme.shapes.card))
                    .background(if (movingToCollection) colors.accent else colors.danger)
                    .padding(horizontal = ClipmoTheme.spacing.md),
                contentAlignment = if (movingToCollection) Alignment.CenterEnd else Alignment.CenterStart,
            ) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    ClipmoIcon(if (movingToCollection) ClipmoIconKind.COLLECTION else ClipmoIconKind.DELETE, Color.White, Modifier.size(18.dp))
                    Spacer(Modifier.width(ClipmoTheme.spacing.sm))
                    androidx.compose.foundation.text.BasicText(
                        if (movingToCollection) "Collection" else "Delete",
                        style = ClipmoTheme.typography.label.copy(color = Color.White),
                    )
                }
            }
        }
        Box(
            Modifier
                .offset { IntOffset(offsetX.roundToInt(), 0) }
                .pointerInput(enabled) {
                    detectHorizontalDragGestures(
                        onHorizontalDrag = { change, dragAmount ->
                            change.consume()
                            offsetX = (offsetX + dragAmount).coerceIn(-threshold * 2f, threshold * 2f)
                        },
                        onDragEnd = {
                            when {
                                offsetX > threshold -> {
                                    onDelete(clip)
                                    offsetX = 0f
                                }
                                offsetX < -threshold -> {
                                    onChooseCollection(clip)
                                    scope.launch { animate(offsetX, 0f) { value, _ -> offsetX = value } }
                                }
                                else -> scope.launch { animate(offsetX, 0f) { value, _ -> offsetX = value } }
                            }
                        },
                    )
                },
        ) {
            content()
        }
    }
}

@Composable
private fun ClipmoClipboardCard(
    clip: ClipRecord,
    onCopy: (ClipRecord) -> Unit,
    onFavorite: (ClipRecord) -> Unit,
    onDelete: (ClipRecord) -> Unit,
    selected: Boolean = false,
    selectionMode: Boolean = false,
    onSelect: (() -> Unit)? = null,
    onOpen: ((ClipRecord) -> Unit)? = null,
) {
    val colors = ClipmoTheme.colors
    val previewPath = remember(clip.assetPaths, clip.kind) {
        if (clip.kind == ClipKind.IMAGE) {
            val assets = clip.assetPaths?.lineSequence()?.filter(String::isNotBlank)?.toList().orEmpty()
            assets.firstOrNull { it.endsWith("thumb.jpg", ignoreCase = true) } ?: assets.firstOrNull()
        } else null
    }
    // Decoding used to run inline during composition, which stalled frames
    // while flinging through image-heavy history; it now happens on IO with a
    // shared LruCache, and the kind icon acts as the placeholder until ready.
    val imageBitmap by produceState<androidx.compose.ui.graphics.ImageBitmap?>(null, previewPath) {
        val path = previewPath ?: return@produceState
        ClipmoThumbCache[path]?.let {
            value = it.asImageBitmap()
            return@produceState
        }
        val decoded = withContext(Dispatchers.IO) {
            runCatching { android.graphics.BitmapFactory.decodeFile(path) }.getOrNull()
        }
        if (decoded != null) {
            ClipmoThumbCache[path] = decoded
            value = decoded.asImageBitmap()
        }
    }
    val timeLabel = remember(clip.timestamp) { ClipmoTimeFormat.short.format(Date(clip.timestamp)) }
    Row(
        Modifier
            .padding(horizontal = ClipmoTheme.spacing.md, vertical = ClipmoTheme.spacing.xxs)
            .fillMaxWidth()
            .heightIn(min = ClipmoTheme.dimensions.clipMinHeight)
            .clip(RoundedCornerShape(ClipmoTheme.shapes.card))
            .background(if (selected) colors.surfacePressed else colors.surfaceRaised)
            .border(
                1.dp,
                if (selected) colors.accent else Color.Transparent,
                RoundedCornerShape(ClipmoTheme.shapes.card),
            )
            .combinedClickable(
                onClick = {
                    if (selectionMode && onSelect != null) onSelect() else onOpen?.invoke(clip) ?: onCopy(clip)
                },
                onLongClick = onSelect ?: { onDelete(clip) },
            )
            .padding(start = ClipmoTheme.spacing.sm, end = ClipmoTheme.spacing.xs, top = ClipmoTheme.spacing.sm, bottom = ClipmoTheme.spacing.sm),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            Modifier
                .size(ClipmoTheme.dimensions.thumbnail)
                .clip(RoundedCornerShape(10.dp))
                .background(colors.surface),
            contentAlignment = Alignment.Center,
        ) {
            val bitmap = imageBitmap
            when {
                selectionMode -> ClipmoIcon(
                    ClipmoIconKind.CHECK,
                    if (selected) colors.accent else colors.textMuted,
                    Modifier.size(18.dp),
                )
                bitmap != null -> Image(
                    bitmap = bitmap,
                    contentDescription = "Image preview",
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.fillMaxSize(),
                )
                else -> ClipmoIcon(iconFor(clip.kind), if (clip.kind == ClipKind.URL) colors.accent else colors.textSecondary, Modifier.size(17.dp))
            }
        }
        Spacer(Modifier.width(ClipmoTheme.spacing.sm))
        Column(Modifier.weight(1f)) {
            androidx.compose.foundation.text.BasicText(
                clipPreview(clip),
                style = ClipmoTheme.typography.body.copy(color = colors.textPrimary),
            )
            Spacer(Modifier.height(ClipmoTheme.spacing.xxs))
            androidx.compose.foundation.text.BasicText(
                "${kindLabel(clip.kind)}  ·  $timeLabel",
                style = ClipmoTheme.typography.metadata.copy(color = colors.textMuted),
            )
        }
        if (!selectionMode) {
            ClipmoIconButton(ClipmoIconKind.STAR, if (clip.favorite) "Unstar" else "Star", { onFavorite(clip) }, if (clip.favorite) colors.accent else colors.textMuted)
            ClipmoIconButton(ClipmoIconKind.COPY, "Copy", { onCopy(clip) })
        }
    }
}

@Composable
private fun ClipmoCollectionCard(name: String, icon: ClipmoIconKind, count: Int, selected: Boolean, modifier: Modifier, onClick: () -> Unit) {
    val colors = ClipmoTheme.colors
    Column(modifier.combinedClickable(onClick = onClick)) {
        Column(
            Modifier
                .fillMaxWidth()
                .height(92.dp)
                .clip(RoundedCornerShape(ClipmoTheme.shapes.card))
                .background(if (selected) colors.surfacePressed else colors.surfaceRaised)
                .border(1.dp, if (selected) colors.accent else colors.border, RoundedCornerShape(ClipmoTheme.shapes.card))
                .padding(ClipmoTheme.spacing.md),
            verticalArrangement = Arrangement.SpaceBetween,
        ) {
            ClipmoIcon(icon, if (selected) colors.accent else colors.textSecondary, Modifier.size(17.dp))
            Column {
                androidx.compose.foundation.text.BasicText(count.toString(), style = ClipmoTheme.typography.title.copy(color = colors.textPrimary))
                androidx.compose.foundation.text.BasicText(if (count == 1) "Clip" else "Clips", style = ClipmoTheme.typography.metadata.copy(color = colors.textMuted))
            }
        }
        Spacer(Modifier.height(ClipmoTheme.spacing.xs))
        androidx.compose.foundation.text.BasicText(name, style = ClipmoTheme.typography.label.copy(color = colors.textSecondary))
    }
}

@Composable
private fun ClipmoDeviceCard(
    name: String,
    platformLabel: String,
    statusLabel: String,
    online: Boolean,
    action: String? = null,
    onAction: (() -> Unit)? = null,
) {
    val colors = ClipmoTheme.colors
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(ClipmoTheme.shapes.card)).background(colors.surfaceRaised).padding(ClipmoTheme.spacing.md),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(48.dp).clip(RoundedCornerShape(14.dp)).background(colors.surface), contentAlignment = Alignment.Center) {
            ClipmoIcon(ClipmoIconKind.DEVICE, if (online) colors.accent else colors.textMuted, Modifier.size(22.dp))
        }
        Spacer(Modifier.width(ClipmoTheme.spacing.md))
        Column(Modifier.weight(1f)) {
            androidx.compose.foundation.text.BasicText(
                name,
                style = ClipmoTheme.typography.body.copy(color = colors.textPrimary),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Spacer(Modifier.height(ClipmoTheme.spacing.xxs))
            Row(verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(7.dp).clip(RoundedCornerShape(50)).background(if (online) colors.accent else colors.textMuted))
                Spacer(Modifier.width(ClipmoTheme.spacing.xs))
                androidx.compose.foundation.text.BasicText(
                    "$platformLabel · $statusLabel",
                    style = ClipmoTheme.typography.metadata.copy(color = if (online) colors.textSecondary else colors.textMuted),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        if (action != null && onAction != null) {
            Spacer(Modifier.width(ClipmoTheme.spacing.sm))
            ClipmoPill(action, false, onClick = onAction)
        }
    }
}

@Composable
private fun ClipmoSettingsGroup(title: String, content: @Composable () -> Unit) {
    val colors = ClipmoTheme.colors
    androidx.compose.foundation.text.BasicText(title, style = ClipmoTheme.typography.section.copy(color = colors.textSecondary))
    Spacer(Modifier.height(ClipmoTheme.spacing.sm))
    Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(ClipmoTheme.shapes.panel)).background(colors.surface).border(1.dp, colors.border, RoundedCornerShape(ClipmoTheme.shapes.panel)).padding(ClipmoTheme.spacing.md)) { content() }
}

@Composable
private fun ClipmoSettingsToggle(title: String, subtitle: String, checked: Boolean, onChanged: (Boolean) -> Unit) {
    Row(Modifier.fillMaxWidth().heightIn(min = 56.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            androidx.compose.foundation.text.BasicText(title, style = ClipmoTheme.typography.body.copy(color = ClipmoTheme.colors.textPrimary))
            Spacer(Modifier.height(ClipmoTheme.spacing.xxs))
            androidx.compose.foundation.text.BasicText(subtitle, style = ClipmoTheme.typography.metadata.copy(color = ClipmoTheme.colors.textMuted))
        }
        Spacer(Modifier.width(ClipmoTheme.spacing.md))
        ClipmoToggle(checked, onChanged)
    }
}

@Composable
private fun ClipmoToggle(checked: Boolean, onChanged: (Boolean) -> Unit) {
    val colors = ClipmoTheme.colors
    val track by animateColorAsState(if (checked) colors.accent else colors.surfacePressed, tween(150), label = "toggle")
    val position by animateFloatAsState(if (checked) 24f else 4f, tween(150), label = "thumb")
    Box(
        Modifier
            .width(52.dp)
            .height(32.dp)
            .clip(RoundedCornerShape(50))
            .background(track)
            .semantics { role = Role.Switch; contentDescription = if (checked) "On" else "Off" }
            .combinedClickable(onClick = { onChanged(!checked) }),
    ) {
        Box(Modifier.padding(start = position.dp, top = 4.dp).size(24.dp).clip(RoundedCornerShape(50)).background(if (checked) Color.White else colors.textSecondary))
    }
}

@Composable
private fun ClipmoActionRow(title: String, subtitle: String, color: Color, onClick: () -> Unit) {
    Row(Modifier.fillMaxWidth().combinedClickable(onClick = onClick), verticalAlignment = Alignment.CenterVertically) {
        ClipmoIcon(ClipmoIconKind.DELETE, color, Modifier.size(17.dp))
        Spacer(Modifier.width(ClipmoTheme.spacing.sm))
        Column {
            androidx.compose.foundation.text.BasicText(title, style = ClipmoTheme.typography.body.copy(color = color))
            androidx.compose.foundation.text.BasicText(subtitle, style = ClipmoTheme.typography.metadata.copy(color = ClipmoTheme.colors.textMuted))
        }
    }
}

@Composable
private fun ClipmoBottomBar(selected: ClipmoScreen, onSelected: (ClipmoScreen) -> Unit) {
    val colors = ClipmoTheme.colors
    Row(
        Modifier.fillMaxWidth().height(64.dp).background(colors.background).border(1.dp, colors.border).padding(horizontal = ClipmoTheme.spacing.sm),
        horizontalArrangement = Arrangement.SpaceAround,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        listOf(
            Triple(ClipmoScreen.HISTORY, ClipmoIconKind.CLIPBOARD, "Clips"),
            Triple(ClipmoScreen.COLLECTIONS, ClipmoIconKind.COLLECTION, "Collections"),
            Triple(ClipmoScreen.DEVICES, ClipmoIconKind.DEVICE, "Devices"),
        ).forEach { (screen, icon, label) ->
            val active = selected == screen
            Column(
                Modifier.weight(1f).fillMaxHeight().combinedClickable(onClick = { onSelected(screen) }),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center,
            ) {
                Box(
                    Modifier.size(width = 56.dp, height = 30.dp).clip(RoundedCornerShape(15.dp)).background(if (active) colors.accentMuted else Color.Transparent),
                    contentAlignment = Alignment.Center,
                ) {
                    ClipmoIcon(icon, if (active) colors.accent else colors.textMuted, Modifier.size(18.dp))
                }
                Spacer(Modifier.height(2.dp))
                androidx.compose.foundation.text.BasicText(label, style = ClipmoTheme.typography.metadata.copy(color = if (active) colors.textPrimary else colors.textMuted))
            }
        }
    }
}

@Composable
private fun ClipmoIconButton(kind: ClipmoIconKind, description: String, onClick: () -> Unit, tint: Color = ClipmoTheme.colors.textSecondary) {
    // No ripple indication: these repeat on every row and the extra
    // indication nodes showed up in fling profiling.
    val interaction = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }
    Box(
        Modifier
            .requiredSize(ClipmoTheme.dimensions.touch)
            .clip(RoundedCornerShape(ClipmoTheme.shapes.pill))
            .combinedClickable(interactionSource = interaction, indication = null, onClick = onClick)
            .semantics { contentDescription = description },
        contentAlignment = Alignment.Center,
    ) { ClipmoIcon(kind, tint, Modifier.size(ClipmoTheme.dimensions.icon)) }
}

@Composable
private fun ClipmoEmptyState(title: String, message: String) {
    Column(
        Modifier.fillMaxWidth().padding(horizontal = 32.dp, vertical = 48.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Image(
            painter = painterResource(R.drawable.clipmo_logo),
            contentDescription = null,
            modifier = Modifier.size(56.dp).clip(RoundedCornerShape(14.dp)),
        )
        Spacer(Modifier.height(ClipmoTheme.spacing.md))
        androidx.compose.foundation.text.BasicText(
            title,
            style = ClipmoTheme.typography.section.copy(fontWeight = FontWeight.SemiBold, color = ClipmoTheme.colors.textPrimary),
        )
        Spacer(Modifier.height(ClipmoTheme.spacing.xs))
        androidx.compose.foundation.text.BasicText(
            message,
            style = ClipmoTheme.typography.body.copy(color = ClipmoTheme.colors.textMuted),
        )
    }
}

@Composable
private fun ClipmoCreateCollectionOverlay(
    onDismiss: () -> Unit,
    onCreate: (String) -> Unit,
) {
    var name by remember { mutableStateOf("") }
    val colors = ClipmoTheme.colors
    Box(Modifier.fillMaxSize().background(colors.scrim).combinedClickable(onClick = onDismiss), contentAlignment = Alignment.Center) {
        Column(
            Modifier
                .widthIn(max = 320.dp)
                .padding(ClipmoTheme.spacing.xl)
                .clip(RoundedCornerShape(ClipmoTheme.shapes.panel))
                .background(colors.surfaceRaised)
                .border(1.dp, colors.border, RoundedCornerShape(ClipmoTheme.shapes.panel))
                .combinedClickable(onClick = {})
                .padding(ClipmoTheme.spacing.lg),
        ) {
            androidx.compose.foundation.text.BasicText("New collection", style = ClipmoTheme.typography.title.copy(color = colors.textPrimary))
            Spacer(Modifier.height(ClipmoTheme.spacing.md))
            ClipmoSearchBar(name, "Collection name", { name = it.take(40) }, searchIcon = false)
            Spacer(Modifier.height(ClipmoTheme.spacing.lg))
            Row(horizontalArrangement = Arrangement.spacedBy(ClipmoTheme.spacing.sm)) {
                ClipmoButton("Cancel", ClipmoButtonStyle.GHOST, Modifier.weight(1f), onClick = onDismiss)
                ClipmoButton("Create", ClipmoButtonStyle.PRIMARY, Modifier.weight(1f), icon = ClipmoIconKind.PLUS) {
                    if (name.isNotBlank()) onCreate(name.trim())
                }
            }
        }
    }
}

@Composable
private fun ClipmoCollectionPickerOverlay(
    collections: List<String>,
    selectedCount: Int,
    onDismiss: () -> Unit,
    onCollection: (String) -> Unit,
) {
    val colors = ClipmoTheme.colors
    Box(Modifier.fillMaxSize().background(colors.scrim).combinedClickable(onClick = onDismiss), contentAlignment = Alignment.Center) {
        Column(
            Modifier
                .widthIn(max = 340.dp)
                .padding(ClipmoTheme.spacing.xl)
                .clip(RoundedCornerShape(ClipmoTheme.shapes.panel))
                .background(colors.surfaceRaised)
                .border(1.dp, colors.border, RoundedCornerShape(ClipmoTheme.shapes.panel))
                .combinedClickable(onClick = {})
                .padding(ClipmoTheme.spacing.lg),
        ) {
            androidx.compose.foundation.text.BasicText("Add to collection", style = ClipmoTheme.typography.title.copy(color = colors.textPrimary))
            Spacer(Modifier.height(ClipmoTheme.spacing.xs))
            androidx.compose.foundation.text.BasicText(
                if (selectedCount == 1) "Choose a collection for this clip." else "Choose a collection for $selectedCount clips.",
                style = ClipmoTheme.typography.metadata.copy(color = colors.textMuted),
            )
            Spacer(Modifier.height(ClipmoTheme.spacing.md))
            if (collections.isEmpty()) {
                androidx.compose.foundation.text.BasicText(
                    "Create a collection from the Collections tab first.",
                    style = ClipmoTheme.typography.body.copy(color = colors.textSecondary),
                )
            } else {
                LazyColumn(Modifier.heightIn(max = 320.dp), verticalArrangement = Arrangement.spacedBy(ClipmoTheme.spacing.xs)) {
                    items(collections, key = { it }) { collection ->
                        Row(
                            Modifier
                                .fillMaxWidth()
                                .clip(RoundedCornerShape(ClipmoTheme.shapes.card))
                                .background(colors.surface)
                                .combinedClickable(onClick = { onCollection(collection) })
                                .padding(ClipmoTheme.spacing.md),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            ClipmoIcon(ClipmoIconKind.COLLECTION, colors.accent, Modifier.size(17.dp))
                            Spacer(Modifier.width(ClipmoTheme.spacing.sm))
                            androidx.compose.foundation.text.BasicText(collection, style = ClipmoTheme.typography.body.copy(color = colors.textPrimary))
                        }
                    }
                }
            }
            Spacer(Modifier.height(ClipmoTheme.spacing.md))
            ClipmoButton("Close", ClipmoButtonStyle.GHOST, Modifier.fillMaxWidth(), onClick = onDismiss)
        }
    }
}

@Composable
private fun ClipmoDetailSheet(
    clip: ClipRecord,
    onCopy: (ClipRecord) -> Unit,
    onDelete: (ClipRecord) -> Unit,
    onEdit: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    val colors = ClipmoTheme.colors
    val editable = clip.kind == ClipKind.TEXT || clip.kind == ClipKind.URL
    var editing by remember { mutableStateOf(false) }
    var draft by remember(clip.id) { mutableStateOf(clip.content) }
    BackHandler {
        if (editing) {
            editing = false
            draft = clip.content
        } else onDismiss()
    }
    val previewPath = remember(clip.assetPaths, clip.kind) {
        if (clip.kind == ClipKind.IMAGE) {
            val assets = clip.assetPaths?.lineSequence()?.filter(String::isNotBlank)?.toList().orEmpty()
            assets.firstOrNull { it.endsWith("thumb.jpg", ignoreCase = true) } ?: assets.firstOrNull()
        } else null
    }
    val imageBitmap by produceState<androidx.compose.ui.graphics.ImageBitmap?>(null, previewPath) {
        val path = previewPath ?: return@produceState
        ClipmoThumbCache[path]?.let {
            value = it.asImageBitmap()
            return@produceState
        }
        val decoded = withContext(Dispatchers.IO) {
            runCatching { android.graphics.BitmapFactory.decodeFile(path) }.getOrNull()
        }
        if (decoded != null) {
            ClipmoThumbCache[path] = decoded
            value = decoded.asImageBitmap()
        }
    }
    Box(Modifier.fillMaxSize().combinedClickable(onClick = onDismiss)) {
        Column(
            Modifier
                .align(Alignment.BottomCenter)
                .fillMaxWidth()
                .fillMaxHeight(0.86f)
                .clip(RoundedCornerShape(topStart = ClipmoTheme.shapes.sheet, topEnd = ClipmoTheme.shapes.sheet))
                .background(colors.surface)
                .combinedClickable(enabled = true, onClick = {}) // keep taps inside off the scrim
                .windowInsetsPadding(WindowInsets.ime)
                .padding(bottom = ClipmoTheme.spacing.sm),
        ) {
            Box(
                Modifier.align(Alignment.CenterHorizontally).padding(top = ClipmoTheme.spacing.xs)
                    .size(width = 36.dp, height = 4.dp).clip(RoundedCornerShape(50)).background(colors.border),
            )
            Row(
                Modifier.fillMaxWidth().padding(horizontal = ClipmoTheme.spacing.lg, vertical = ClipmoTheme.spacing.sm),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Box(
                    Modifier.size(40.dp).clip(RoundedCornerShape(14.dp)).background(colors.accentMuted),
                    contentAlignment = Alignment.Center,
                ) {
                    ClipmoIcon(iconFor(clip.kind), colors.accent, Modifier.size(20.dp))
                }
                Spacer(Modifier.width(ClipmoTheme.spacing.md))
                Column {
                    androidx.compose.foundation.text.BasicText(kindLabel(clip.kind), style = ClipmoTheme.typography.title.copy(color = colors.textPrimary))
                    androidx.compose.foundation.text.BasicText(
                        ClipmoTimeFormat.short.format(Date(clip.timestamp)),
                        style = ClipmoTheme.typography.metadata.copy(color = colors.textMuted),
                    )
                }
                Spacer(Modifier.weight(1f))
                if (clip.favorite) ClipmoIcon(ClipmoIconKind.STAR, colors.accent, Modifier.size(20.dp))
            }
            Box(
                Modifier
                    .weight(1f)
                    .fillMaxWidth()
                    .padding(horizontal = ClipmoTheme.spacing.lg)
                    .clip(RoundedCornerShape(ClipmoTheme.shapes.search))
                    .background(colors.surfaceRaised),
            ) {
                if (editing) {
                    BasicTextField(
                        value = draft,
                        onValueChange = { draft = it },
                        textStyle = ClipmoTheme.typography.body.copy(color = colors.textPrimary),
                        cursorBrush = SolidColor(colors.accent),
                        modifier = Modifier
                            .fillMaxSize()
                            .padding(ClipmoTheme.spacing.md)
                            .verticalScroll(rememberScrollState()),
                    )
                } else {
                    Column(
                        Modifier
                            .fillMaxSize()
                            .padding(ClipmoTheme.spacing.md)
                            .verticalScroll(rememberScrollState()),
                    ) {
                        val bitmap = imageBitmap
                        if (clip.kind == ClipKind.IMAGE && bitmap != null) {
                            Image(
                                bitmap = bitmap,
                                contentDescription = "Image preview",
                                contentScale = ContentScale.Fit,
                                modifier = Modifier.fillMaxWidth(),
                            )
                        } else {
                            androidx.compose.foundation.text.BasicText(
                                clip.content,
                                style = ClipmoTheme.typography.body.copy(color = colors.textSecondary),
                            )
                        }
                    }
                }
            }
            Column(Modifier.fillMaxWidth().padding(horizontal = ClipmoTheme.spacing.lg, vertical = ClipmoTheme.spacing.md)) {
                if (editing) {
                    Row(horizontalArrangement = Arrangement.spacedBy(ClipmoTheme.spacing.sm)) {
                        ClipmoButton("Cancel", ClipmoButtonStyle.GHOST, Modifier.weight(1f)) {
                            editing = false
                            draft = clip.content
                        }
                        ClipmoButton(
                            "Save",
                            ClipmoButtonStyle.PRIMARY,
                            Modifier.weight(1f),
                            icon = ClipmoIconKind.CHECK,
                        ) { onEdit(draft.trim()) }
                    }
                } else {
                    // Primary actions live in the thumb zone; Copy leads.
                    Row(horizontalArrangement = Arrangement.spacedBy(ClipmoTheme.spacing.sm)) {
                        ClipmoButton("Copy", ClipmoButtonStyle.PRIMARY, Modifier.weight(if (editable) 1.5f else 1f), icon = ClipmoIconKind.COPY) { onCopy(clip) }
                        if (editable) {
                            ClipmoButton("Edit", ClipmoButtonStyle.SECONDARY, Modifier.weight(1f), icon = ClipmoIconKind.EDIT) { editing = true }
                        }
                    }
                    Spacer(Modifier.height(ClipmoTheme.spacing.sm))
                    Row(horizontalArrangement = Arrangement.spacedBy(ClipmoTheme.spacing.sm)) {
                        ClipmoButton("Delete", ClipmoButtonStyle.GHOST, Modifier.weight(1f), icon = ClipmoIconKind.DELETE, tint = colors.danger) { onDelete(clip) }
                        ClipmoButton("Close", ClipmoButtonStyle.GHOST, Modifier.weight(1f), icon = ClipmoIconKind.CLOSE, onClick = onDismiss)
                    }
                }
            }
        }
    }
}

@Composable
private fun ClipmoConfirmOverlay(
    title: String,
    message: String,
    confirmLabel: String,
    onDismiss: () -> Unit,
    onConfirm: () -> Unit,
) {
    val colors = ClipmoTheme.colors
    Box(Modifier.fillMaxSize().background(colors.scrim).combinedClickable(onClick = onDismiss), contentAlignment = Alignment.Center) {
        Column(
            Modifier.widthIn(max = 320.dp).padding(ClipmoTheme.spacing.xl).clip(RoundedCornerShape(ClipmoTheme.shapes.panel)).background(colors.surfaceRaised).border(1.dp, colors.border, RoundedCornerShape(ClipmoTheme.shapes.panel)).combinedClickable(onClick = {}).padding(ClipmoTheme.spacing.lg),
        ) {
            androidx.compose.foundation.text.BasicText(title, style = ClipmoTheme.typography.title.copy(color = colors.textPrimary))
            Spacer(Modifier.height(ClipmoTheme.spacing.sm))
            androidx.compose.foundation.text.BasicText(message, style = ClipmoTheme.typography.body.copy(color = colors.textSecondary), maxLines = 3, overflow = TextOverflow.Ellipsis)
            Spacer(Modifier.height(ClipmoTheme.spacing.lg))
            Row(horizontalArrangement = Arrangement.spacedBy(ClipmoTheme.spacing.sm)) {
                ClipmoButton("Cancel", ClipmoButtonStyle.GHOST, Modifier.weight(1f), onClick = onDismiss)
                ClipmoButton(confirmLabel, ClipmoButtonStyle.DANGER, Modifier.weight(1f), icon = ClipmoIconKind.DELETE, onClick = onConfirm)
            }
        }
    }
}

private fun sourceLabel(source: String?): String = when {
    source.isNullOrBlank() || source == "clipboard" -> "This Android phone"
    else -> source.substringAfterLast('.').replaceFirstChar { it.uppercase() }
}
private fun relativeSeen(ms: Long): String {
    val diff = System.currentTimeMillis() - ms
    return when {
        diff < 60_000 -> "just now"
        diff < 3_600_000 -> "${diff / 60_000}m ago"
        diff < 86_400_000 -> "${diff / 3_600_000}h ago"
        else -> "${diff / 86_400_000}d ago"
    }
}
private fun kindLabel(kind: ClipKind): String = when (kind) { ClipKind.TEXT -> "Text"; ClipKind.URL -> "Link"; ClipKind.IMAGE -> "Image"; ClipKind.FILE -> "File" }
private fun iconFor(kind: ClipKind): ClipmoIconKind = when (kind) { ClipKind.TEXT -> ClipmoIconKind.TEXT; ClipKind.URL -> ClipmoIconKind.LINK; ClipKind.IMAGE -> ClipmoIconKind.IMAGE; ClipKind.FILE -> ClipmoIconKind.FILE }

private fun ClipRecord.isLocalTo(localDeviceId: String): Boolean =
    if (!originDevice.isNullOrBlank()) originDevice == localDeviceId
    else source.isNullOrBlank() || source == "clipboard"
// Line-count-constrained text (maxLines) hits a very slow layout path on
// several OEM font stacks, freezing flings in long lists; previews are
// truncated in the string instead and the full text opens in the detail view.
private const val CLIP_PREVIEW_MAX_CHARS = 90

private fun clipPreview(clip: ClipRecord): String = when (clip.kind) {
    ClipKind.IMAGE -> "Image"
    ClipKind.FILE -> clip.content.substringAfterLast('/')
    else -> clip.content.trim().let { if (it.length > CLIP_PREVIEW_MAX_CHARS) it.take(CLIP_PREVIEW_MAX_CHARS).trimEnd() + "…" else it }
}
