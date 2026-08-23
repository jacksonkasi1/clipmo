@file:OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)

package app.clipdeck.desktop.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animate
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
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
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.ExperimentalMaterialApi
import androidx.compose.material.pullrefresh.PullRefreshIndicator
import androidx.compose.material.pullrefresh.pullRefresh
import androidx.compose.material.pullrefresh.rememberPullRefreshState
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
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.clipdeck.desktop.R
import app.clipdeck.desktop.data.ClipKind
import app.clipdeck.desktop.data.ClipRecord
import app.clipdeck.desktop.data.TrustedDeviceRecord
import app.clipdeck.desktop.ui.theme.ClipmoTheme
import app.clipdeck.desktop.ui.theme.ClipmoThemeMode
import java.text.DateFormat
import java.util.Date
import kotlin.math.roundToInt
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private enum class ClipmoScreen { HISTORY, COLLECTIONS, DEVICES, SETTINGS }
private enum class ClipFilter(val label: String, val icon: ClipmoIconKind) {
    ALL("All", ClipmoIconKind.CLIPBOARD),
    TEXT("Text", ClipmoIconKind.TEXT),
    LINKS("Links", ClipmoIconKind.LINK),
    IMAGES("Images", ClipmoIconKind.IMAGE),
    FILES("Files", ClipmoIconKind.FILE),
    STARRED("Starred", ClipmoIconKind.STAR),
}

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
    onDeleteCollection: (String) -> Unit = {},
    onAddToCollection: (Set<Long>, String) -> Unit,
    onRemoveFromCollection: (Set<Long>, String) -> Unit = { _, _ -> },
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
    onRefresh: () -> Unit = {},
    onSyncNow: () -> Unit = onRefresh,
) {
    ClipmoTheme(state.themeMode) {
        var screen by rememberSaveable { mutableStateOf(ClipmoScreen.HISTORY) }
        var pendingDelete by remember { mutableStateOf<ClipRecord?>(null) }
        var pendingDeleteCollection by remember { mutableStateOf<String?>(null) }
        var pendingClear by remember { mutableStateOf(false) }
        var copyPulse by remember { mutableStateOf(0) }
        var detailClip by remember { mutableStateOf<ClipRecord?>(null) }
        var sheetOpen by remember { mutableStateOf(false) }
        var lastDetail by remember { mutableStateOf<ClipRecord?>(null) }
        val haptic = LocalHapticFeedback.current

        LaunchedEffect(detailClip) {
            detailClip?.let { lastDetail = it }
            sheetOpen = detailClip != null
        }

        val copyWithFeedback: (ClipRecord) -> Unit = remember(onCopy) {
            { clip ->
                haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                onCopy(clip)
                copyPulse++
            }
        }
        val colors = ClipmoTheme.colors

        Box(
            Modifier
                .fillMaxSize()
                .background(colors.background),
        ) {
            Column(
                Modifier
                    .fillMaxSize()
                    .windowInsetsPadding(WindowInsets.safeDrawing),
            ) {
                ClipmoTopBar(
                    screen = screen,
                    onSettings = { screen = ClipmoScreen.SETTINGS },
                    onBack = if (screen == ClipmoScreen.SETTINGS) ({ screen = ClipmoScreen.HISTORY }) else null,
                )
                Box(Modifier.weight(1f)) {
                    when (screen) {
                        ClipmoScreen.HISTORY -> HistoryScreen(
                            state = state,
                            onCopy = copyWithFeedback,
                            onFavorite = {
                                haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                                onFavorite(it)
                            },
                            onDelete = onDelete,
                            onFavoriteMany = onFavoriteMany,
                            onDeleteMany = onDeleteMany,
                            onAddToCollection = onAddToCollection,
                            onOpenClip = { detailClip = it },
                            onRefresh = onRefresh,
                        )
                        ClipmoScreen.COLLECTIONS -> CollectionsScreen(
                            state = state,
                            onCopy = copyWithFeedback,
                            onFavorite = {
                                haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                                onFavorite(it)
                            },
                            onDelete = { pendingDelete = it },
                            onCreateCollection = onCreateCollection,
                            onDeleteCollectionRequest = { pendingDeleteCollection = it },
                            onRemoveFromCollection = onRemoveFromCollection,
                            onOpenClip = { detailClip = it },
                        )
                        ClipmoScreen.DEVICES -> DevicesScreen(
                            state = state,
                            onForgetDevice = onForgetDevice,
                            onSyncChanged = onSyncChanged,
                            onStartPairing = onStartPairing,
                            onSyncNow = onSyncNow,
                        )
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
                    message = clip.content.take(120),
                    confirmLabel = "Delete",
                    onDismiss = { pendingDelete = null },
                    onConfirm = {
                        onDelete(clip)
                        pendingDelete = null
                    },
                )
            }
            pendingDeleteCollection?.let { tag ->
                ClipmoConfirmOverlay(
                    title = "Delete collection \"$tag\"?",
                    message = "Clips will remain in your history, but will be removed from this collection.",
                    confirmLabel = "Delete",
                    onDismiss = { pendingDeleteCollection = null },
                    onConfirm = {
                        onDeleteCollection(tag)
                        pendingDeleteCollection = null
                    },
                )
            }
            if (pendingClear) {
                ClipmoConfirmOverlay(
                    title = "Clear clipboard history?",
                    message = "This permanently removes every local clip and cannot be undone.",
                    confirmLabel = "Clear all",
                    onDismiss = { pendingClear = false },
                    onConfirm = {
                        onClear()
                        pendingClear = false
                    },
                )
            }

            AnimatedVisibility(
                sheetOpen,
                enter = fadeIn(tween(160)),
                exit = fadeOut(tween(160)),
            ) {
                Box(
                    Modifier
                        .fillMaxSize()
                        .background(ClipmoTheme.colors.scrim)
                        .combinedClickable(
                            interactionSource = remember { MutableInteractionSource() },
                            indication = null,
                            onClick = { detailClip = null },
                        ),
                )
            }

            val activeDetailClip = remember(lastDetail, state.clips) {
                lastDetail?.let { last ->
                    state.clips.find { it.id == last.id } ?: last
                }
            }

            AnimatedVisibility(
                sheetOpen,
                enter = slideInVertically(spring(dampingRatio = 0.82f, stiffness = 420f)) { it },
                exit = slideOutVertically(tween(180)) { it },
                modifier = Modifier.align(Alignment.BottomCenter),
            ) {
                activeDetailClip?.let { clip ->
                    ClipmoDetailSheet(
                        clip = clip,
                        collections = state.collections,
                        onCopy = copyWithFeedback,
                        onFavorite = { clipToToggle ->
                            haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                            val toggled = clipToToggle.copy(favorite = !clipToToggle.favorite)
                            lastDetail = toggled
                            detailClip = toggled
                            onFavorite(clipToToggle)
                        },
                        onDelete = {
                            onDelete(clip)
                            detailClip = null
                        },
                        onEdit = { content ->
                            onEditClip(clip.id, content)
                            detailClip = null
                        },
                        onAddToCollection = onAddToCollection,
                        onCreateCollection = onCreateCollection,
                        onRemoveFromCollection = onRemoveFromCollection,
                        onDismiss = { detailClip = null },
                    )
                }
            }

            ClipmoCopiedPill(
                copyPulse = copyPulse,
                modifier = Modifier
                    .align(Alignment.BottomCenter)
                    .windowInsetsPadding(WindowInsets.navigationBars),
            )
        }
    }
}

// -----------------------------------------------------------------------------
// Top Bar
// -----------------------------------------------------------------------------

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
            ClipmoIconButton(
                kind = ClipmoIconKind.BACK,
                description = "Back",
                onClick = onBack,
                tint = colors.textPrimary,
            )
            Spacer(Modifier.width(space.xs))
            BasicText(
                text = if (screen == ClipmoScreen.SETTINGS) "Settings" else "Back",
                style = type.title.copy(color = colors.textPrimary, fontWeight = FontWeight.Bold),
                modifier = Modifier.weight(1f),
            )
        } else {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.weight(1f),
            ) {
                Image(
                    painter = painterResource(R.drawable.clipmo_logo),
                    contentDescription = "Clipmo logo",
                    modifier = Modifier
                        .size(28.dp)
                        .clip(RoundedCornerShape(8.dp)),
                )
                Spacer(Modifier.width(space.sm))
                Column {
                    BasicText(
                        text = "Clipmo",
                        style = type.brand.copy(color = colors.textPrimary),
                    )
                }
            }
            ClipmoIconButton(
                kind = ClipmoIconKind.SETTINGS,
                description = "Settings",
                onClick = onSettings,
                tint = colors.textSecondary,
            )
        }
    }
}

// -----------------------------------------------------------------------------
// History (Main Clips) Screen
// -----------------------------------------------------------------------------

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
    onOpenClip: (ClipRecord) -> Unit,
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

    val devices = remember(clips, state.trustedDevices, state.localDeviceId) {
        listOf(
            Triple("all", "All devices", clips.size),
            Triple(state.localDeviceId, "This phone", clips.count { it.isLocalTo(state.localDeviceId) }),
        ) + state.trustedDevices.map { device ->
            Triple(device.id, device.name, clips.count { it.originDevice == device.id })
        }
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

    val grouped = remember(shown, state.trustedDevices, state.localDeviceId) {
        val trustedNames = state.trustedDevices.associate { it.id to it.name }
        shown.groupBy { clip ->
            when {
                clip.isLocalTo(state.localDeviceId) -> "This phone"
                !clip.originDevice.isNullOrBlank() -> trustedNames[clip.originDevice] ?: sourceLabel(clip.source)
                else -> sourceLabel(clip.source)
            }
        }
    }

    val pullState = rememberPullRefreshState(refreshing, { refreshing = true })

    Box(
        Modifier
            .fillMaxSize()
            .pullRefresh(pullState),
    ) {
        LazyColumn(
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(bottom = ClipmoTheme.spacing.xl),
        ) {
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
                        ClipmoSearchBar(
                            value = query,
                            hint = "Search ${clips.size} clips...",
                            onValueChange = { query = it },
                        )
                    }

                    Spacer(Modifier.height(ClipmoTheme.spacing.sm))

                    // Device Selector: Visually elevated over ordinary chips
                    ClipmoDeviceSelectorRow(
                        devices = devices,
                        selectedId = selectedDevice,
                        localDeviceId = state.localDeviceId,
                        onSelect = { selectedDevice = it },
                    )

                    Spacer(Modifier.height(ClipmoTheme.spacing.xs))

                    // Secondary Content Filters
                    ClipmoFilterRow(
                        selected = filter,
                        onSelected = { filter = it },
                    )

                    Spacer(Modifier.height(ClipmoTheme.spacing.md))
                }
            }

            if (shown.isEmpty()) {
                item {
                    ClipmoEmptyState(
                        title = if (clips.isEmpty()) "Your clipboard is quiet" else "No matching clips",
                        message = if (clips.isEmpty()) "Copy something on this device or sync to see clips here." else "Try adjusting your search query or filters.",
                    )
                }
            } else {
                grouped.forEach { (device, deviceClips) ->
                    item {
                        ClipmoDeviceHeader(device, deviceClips.size)
                    }
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
                                onOpen = { onOpenClip(clip) },
                            )
                        }
                    }
                    item { Spacer(Modifier.height(ClipmoTheme.spacing.sm)) }
                }
            }
        }

        PullRefreshIndicator(
            refreshing = refreshing,
            state = pullState,
            modifier = Modifier.align(Alignment.TopCenter),
            contentColor = ClipmoTheme.colors.accent,
            backgroundColor = ClipmoTheme.colors.surfaceRaised,
        )

        pendingBulkDelete?.let { ids ->
            ClipmoConfirmOverlay(
                title = "Delete ${ids.size} clips?",
                message = "The selected clips will be removed permanently.",
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
    }
}

// -----------------------------------------------------------------------------
// Collections Screen
// -----------------------------------------------------------------------------

@Composable
private fun CollectionsScreen(
    state: ClipmoUiState,
    onCopy: (ClipRecord) -> Unit,
    onFavorite: (ClipRecord) -> Unit,
    onDelete: (ClipRecord) -> Unit,
    onCreateCollection: (String) -> Unit,
    onDeleteCollectionRequest: (String) -> Unit = {},
    onRemoveFromCollection: (Set<Long>, String) -> Unit = { _, _ -> },
    onOpenClip: (ClipRecord) -> Unit = {},
) {
    val clips = state.clips
    val colors = ClipmoTheme.colors
    val space = ClipmoTheme.spacing
    val type = ClipmoTheme.typography

    val tags = remember(clips, state.collections) {
        (state.collections + clips.flatMap(ClipRecord::tags)).distinctBy(String::lowercase).sortedBy(String::lowercase)
    }
    var selected by rememberSaveable { mutableStateOf<String?>(null) }
    var creatingCollection by remember { mutableStateOf(false) }

    LaunchedEffect(tags) {
        if (selected !in tags) selected = tags.firstOrNull()
    }

    val selectedClips = clips.filter { selected != null && selected in it.tags }

    LazyColumn(
        Modifier.fillMaxSize(),
        contentPadding = PaddingValues(bottom = space.xl),
    ) {
        item {
            Column(Modifier.padding(horizontal = space.md)) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Column {
                        BasicText(
                            "Tag collections",
                            style = type.title.copy(color = colors.textPrimary, fontWeight = FontWeight.Bold),
                        )
                        BasicText(
                            "${tags.size} collections",
                            style = type.metadata.copy(color = colors.textMuted),
                        )
                    }
                    ClipmoButton(
                        label = "New collection",
                        style = ClipmoButtonStyle.PRIMARY,
                        icon = ClipmoIconKind.PLUS,
                        onClick = { creatingCollection = true },
                    )
                }

                Spacer(Modifier.height(space.md))

                if (tags.isEmpty()) {
                    ClipmoEmptyState(
                        "No tag collections yet",
                        "Create a collection above, then swipe or open clips to tag them.",
                    )
                } else {
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .horizontalScroll(rememberScrollState()),
                        horizontalArrangement = Arrangement.spacedBy(space.sm),
                    ) {
                        tags.forEach { tag ->
                            ClipmoCollectionCard(
                                name = tag,
                                icon = ClipmoIconKind.COLLECTION,
                                count = clips.count { tag in it.tags },
                                selected = selected == tag,
                                modifier = Modifier.width(148.dp),
                                onDelete = { onDeleteCollectionRequest(tag) },
                            ) { selected = tag }
                        }
                    }
                }

                Spacer(Modifier.height(space.lg))

                selected?.let { currentTag ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = space.xxs, vertical = space.xs),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.SpaceBetween,
                    ) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            ClipmoIcon(ClipmoIconKind.COLLECTION, colors.accent, Modifier.size(16.dp))
                            Spacer(Modifier.width(space.xs))
                            BasicText(
                                currentTag,
                                style = type.titleSmall.copy(color = colors.textPrimary, fontWeight = FontWeight.Bold),
                            )
                            Spacer(Modifier.width(space.xs))
                            BasicText(
                                "(${selectedClips.size})",
                                style = type.metadata.copy(color = colors.textMuted),
                            )
                        }
                        ClipmoButton(
                            label = "Delete collection",
                            style = ClipmoButtonStyle.TONAL_DANGER,
                            icon = ClipmoIconKind.DELETE,
                            onClick = { onDeleteCollectionRequest(currentTag) },
                        )
                    }
                }
            }
        }

        if (tags.isNotEmpty() && selectedClips.isEmpty()) {
            item {
                ClipmoEmptyState(
                    "No clips in this collection",
                    "Swipe left on any clip in the Clips tab to add it here.",
                )
            }
        }

        items(selectedClips, key = { it.id }) { clip ->
            ClipmoClipboardCard(
                clip = clip,
                onCopy = onCopy,
                onFavorite = onFavorite,
                onDelete = onDelete,
                onOpen = onOpenClip,
                onRemoveFromCollection = selected?.let { tag -> { onRemoveFromCollection(setOf(clip.id), tag) } },
            )
        }
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

// -----------------------------------------------------------------------------
// Devices Screen
// -----------------------------------------------------------------------------

@Composable
private fun DevicesScreen(
    state: ClipmoUiState,
    onForgetDevice: (TrustedDeviceRecord) -> Unit,
    onSyncChanged: (Boolean) -> Unit,
    onStartPairing: () -> Unit,
    onSyncNow: () -> Unit,
) {
    val colors = ClipmoTheme.colors
    val space = ClipmoTheme.spacing
    val type = ClipmoTheme.typography

    LazyColumn(
        Modifier.fillMaxSize(),
        contentPadding = PaddingValues(horizontal = space.md, vertical = space.sm),
    ) {
        item {
            // Section 1: Pair a Device
            BasicText(
                "Pair a device",
                style = type.section.copy(color = colors.textSecondary),
            )
            Spacer(Modifier.height(space.xs))

            Column(
                Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(ClipmoTheme.shapes.panel))
                    .background(colors.surfaceContainerLow)
                    .border(1.dp, colors.border, RoundedCornerShape(ClipmoTheme.shapes.panel))
                    .padding(space.md),
            ) {
                BasicText(
                    "Pair Clipmo using this code",
                    style = type.bodyMedium.copy(color = colors.textSecondary),
                )
                Spacer(Modifier.height(space.sm))

                // Emphasized Pairing Code Display
                val formattedCode = state.pairingCode.ifBlank { "------" }
                    .chunked(3)
                    .joinToString("  ")

                Box(
                    Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(ClipmoTheme.shapes.md))
                        .background(colors.surfaceContainer)
                        .border(1.dp, colors.borderSubtle, RoundedCornerShape(ClipmoTheme.shapes.md))
                        .padding(vertical = space.md),
                    contentAlignment = Alignment.Center,
                ) {
                    BasicText(
                        text = formattedCode,
                        style = type.pairingCode.copy(color = colors.accent),
                    )
                }

                Spacer(Modifier.height(space.md))

                Row(horizontalArrangement = Arrangement.spacedBy(space.sm)) {
                    ClipmoButton(
                        label = if (state.pairingModeActive) "Pairing active…" else "Pair device",
                        style = if (state.pairingModeActive) ClipmoButtonStyle.SECONDARY else ClipmoButtonStyle.PRIMARY,
                        modifier = Modifier.weight(1f),
                        icon = ClipmoIconKind.PLUS,
                    ) {
                        if (!state.syncEnabled) onSyncChanged(true)
                        onStartPairing()
                    }
                    ClipmoButton(
                        label = "Sync now",
                        style = ClipmoButtonStyle.SECONDARY,
                        modifier = Modifier.weight(1f),
                        icon = ClipmoIconKind.REFRESH,
                        onClick = onSyncNow,
                    )
                }
            }

            Spacer(Modifier.height(space.xl))

            // Section 2: This Device
            BasicText(
                "This device",
                style = type.section.copy(color = colors.textSecondary),
            )
            Spacer(Modifier.height(space.xs))

            ClipmoDeviceCard(
                name = state.localDeviceName,
                platformLabel = "Android",
                statusLabel = if (state.syncEnabled) "Ready · LAN Sync active" else "Ready · Local only",
                online = true,
                isLocal = true,
            )

            Spacer(Modifier.height(space.xl))

            // Section 3: Paired Devices
            Row(
                Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                BasicText(
                    "Paired devices",
                    style = type.section.copy(color = colors.textSecondary),
                )
                if (state.trustedDevices.isNotEmpty()) {
                    BasicText(
                        "${state.trustedDevices.size} paired",
                        style = type.metadata.copy(color = colors.textMuted),
                    )
                }
            }
            Spacer(Modifier.height(space.xs))

            if (state.trustedDevices.isEmpty()) {
                ClipmoEmptyState(
                    title = if (state.syncEnabled) "Looking for devices" else "LAN sync is off",
                    message = if (state.syncEnabled) {
                        "Open Clipmo on your PC or other phones with the same pairing code."
                    } else {
                        "Tap 'Pair device' or enable sync in Settings to link devices."
                    },
                )
            } else {
                state.trustedDevices.forEach { device ->
                    ClipmoDeviceCard(
                        name = device.name,
                        platformLabel = device.platform.replaceFirstChar(Char::uppercase),
                        statusLabel = if (device.online) "Connected" else "Seen ${relativeSeen(device.lastSeenMs)}",
                        online = device.online,
                        action = "Forget",
                        onAction = { onForgetDevice(device) },
                    )
                    Spacer(Modifier.height(space.xs))
                }
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Settings Screen
// -----------------------------------------------------------------------------

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

    LazyColumn(
        Modifier.fillMaxSize(),
        contentPadding = PaddingValues(horizontal = space.md, vertical = space.sm),
    ) {
        item {
            ClipmoSettingsGroup(title = "Clipboard") {
                ClipmoSettingsToggle(
                    title = "Clipboard monitoring",
                    subtitle = "Capture clipboard changes while Clipmo is running.",
                    checked = state.monitorEnabled,
                    onChanged = onMonitorChanged,
                )
                Spacer(Modifier.height(space.sm))
                ClipmoSettingsToggle(
                    title = "Screenshot capture",
                    subtitle = "Automatically save new screenshots to history while monitoring runs.",
                    checked = state.screenshotCaptureEnabled,
                    onChanged = onScreenshotCaptureChanged,
                )
            }

            Spacer(Modifier.height(space.lg))

            ClipmoSettingsGroup(title = "Local sync") {
                ClipmoSettingsToggle(
                    title = "LAN sync",
                    subtitle = "Automatically discover and reconnect to trusted devices on your local network.",
                    checked = state.syncEnabled,
                    onChanged = onSyncChanged,
                )
                Spacer(Modifier.height(space.sm))
                ClipmoSettingsToggle(
                    title = "Copy live synced clips",
                    subtitle = "When enabled, newly received clips from other devices copy directly to your phone's system clipboard.",
                    checked = state.copyLiveSyncToClipboard,
                    onChanged = onCopyLiveSyncChanged,
                )
                Spacer(Modifier.height(space.md))
                BasicText(
                    "Pairing code",
                    style = ClipmoTheme.typography.label.copy(color = colors.textSecondary),
                )
                Spacer(Modifier.height(space.xs))
                ClipmoSearchBar(
                    value = state.pairingCode,
                    hint = "Enter pairing code",
                    onValueChange = onPairingCodeChanged,
                    searchIcon = false,
                )
            }

            Spacer(Modifier.height(space.lg))

            ClipmoSettingsGroup(title = "Appearance") {
                ClipmoAppearanceSelector(
                    currentMode = state.themeMode,
                    onModeSelected = onThemeChanged,
                )
            }

            Spacer(Modifier.height(space.lg))

            ClipmoSettingsGroup(title = "History") {
                ClipmoActionRow(
                    title = "Clear clipboard history",
                    subtitle = "Remove every locally stored clip from this device",
                    color = colors.danger,
                    onClick = onClear,
                )
            }
            Spacer(Modifier.height(space.xl))
        }
    }
}

// -----------------------------------------------------------------------------
// Core Material 3 Expressive Design Components
// -----------------------------------------------------------------------------

@Composable
private fun ClipmoSearchBar(
    value: String,
    hint: String,
    onValueChange: (String) -> Unit,
    searchIcon: Boolean = true,
) {
    val colors = ClipmoTheme.colors
    val shape = RoundedCornerShape(ClipmoTheme.shapes.search)

    Row(
        Modifier
            .fillMaxWidth()
            .height(ClipmoTheme.dimensions.searchHeight)
            .clip(shape)
            .background(colors.surfaceContainerLow)
            .border(1.dp, colors.border, shape)
            .padding(horizontal = ClipmoTheme.spacing.md),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (searchIcon) {
            ClipmoIcon(ClipmoIconKind.SEARCH, colors.textMuted, Modifier.size(16.dp))
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
                    if (value.isEmpty()) {
                        BasicText(hint, style = ClipmoTheme.typography.body.copy(color = colors.textMuted))
                    }
                    inner()
                }
            },
        )
        if (value.isNotEmpty()) {
            Box(
                Modifier
                    .size(36.dp)
                    .clip(CircleShape)
                    .combinedClickable(onClick = { onValueChange("") }),
                contentAlignment = Alignment.Center,
            ) {
                ClipmoIcon(ClipmoIconKind.CLOSE, colors.textMuted, Modifier.size(16.dp))
            }
        }
    }
}

@Composable
private fun ClipmoDeviceSelectorRow(
    devices: List<Triple<String, String, Int>>,
    selectedId: String,
    localDeviceId: String,
    onSelect: (String) -> Unit,
) {
    val colors = ClipmoTheme.colors
    val type = ClipmoTheme.typography
    val space = ClipmoTheme.spacing

    Row(
        Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState()),
        horizontalArrangement = Arrangement.spacedBy(space.xs),
    ) {
        devices.forEach { (id, name, count) ->
            val selected = selectedId == id
            val isLocal = id == localDeviceId
            val isAll = id == "all"

            val bg by animateColorAsState(
                if (selected) colors.accentMuted else colors.surfaceContainerLow,
                tween(150),
                label = "devBg",
            )
            val borderCol by animateColorAsState(
                if (selected) Color.Transparent else colors.border,
                tween(150),
                label = "devBorder",
            )
            val iconCol by animateColorAsState(
                if (selected) colors.onAccentMuted else colors.textMuted,
                tween(150),
                label = "devIcon",
            )
            val textCol by animateColorAsState(
                if (selected) colors.onAccentMuted else colors.textSecondary,
                tween(150),
                label = "devText",
            )

            Row(
                Modifier
                    .height(38.dp)
                    .clip(RoundedCornerShape(ClipmoTheme.shapes.pill))
                    .background(bg)
                    .border(1.dp, borderCol, RoundedCornerShape(ClipmoTheme.shapes.pill))
                    .combinedClickable(onClick = { onSelect(id) })
                    .padding(horizontal = space.sm, vertical = space.xxs),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                ClipmoIcon(
                    kind = when {
                        isAll -> ClipmoIconKind.DEVICE
                        isLocal -> ClipmoIconKind.PHONE
                        else -> ClipmoIconKind.DESKTOP
                    },
                    color = iconCol,
                    modifier = Modifier.size(15.dp),
                )
                Spacer(Modifier.width(space.xs))
                BasicText(
                    text = name,
                    style = type.label.copy(
                        color = textCol,
                        fontWeight = if (selected) FontWeight.Bold else FontWeight.Medium,
                    ),
                )
                Spacer(Modifier.width(space.xs))
                Box(
                    Modifier
                        .clip(RoundedCornerShape(ClipmoTheme.shapes.pill))
                        .background(if (selected) colors.accent else colors.surfaceContainerHigh)
                        .padding(horizontal = 7.dp, vertical = 2.dp),
                ) {
                    BasicText(
                        text = count.toString(),
                        style = type.labelSmall.copy(
                            color = if (selected) colors.onAccent else colors.textSecondary,
                            fontWeight = FontWeight.Bold,
                        ),
                    )
                }
            }
        }
    }
}

@Composable
private fun ClipmoFilterRow(
    selected: ClipFilter,
    onSelected: (ClipFilter) -> Unit,
) {
    val space = ClipmoTheme.spacing
    Row(
        Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState()),
        horizontalArrangement = Arrangement.spacedBy(space.xs),
    ) {
        ClipFilter.entries.forEach { filter ->
            ClipmoFilterChip(
                label = filter.label,
                icon = filter.icon,
                selected = filter == selected,
                onClick = { onSelected(filter) },
            )
        }
    }
}

@Composable
private fun ClipmoFilterChip(
    label: String,
    icon: ClipmoIconKind,
    selected: Boolean,
    onClick: () -> Unit,
) {
    val colors = ClipmoTheme.colors
    val bg by animateColorAsState(
        if (selected) colors.accentMuted else colors.surfaceContainerLow,
        tween(150),
        label = "chipBg",
    )
    val textCol by animateColorAsState(
        if (selected) colors.onAccentMuted else colors.textSecondary,
        tween(150),
        label = "chipText",
    )
    val borderCol by animateColorAsState(
        if (selected) Color.Transparent else colors.border,
        tween(150),
        label = "chipBorder",
    )

    Row(
        Modifier
            .height(34.dp)
            .clip(RoundedCornerShape(ClipmoTheme.shapes.pill))
            .background(bg)
            .border(1.dp, borderCol, RoundedCornerShape(ClipmoTheme.shapes.pill))
            .combinedClickable(onClick = onClick)
            .padding(horizontal = ClipmoTheme.spacing.md),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        ClipmoIcon(icon, textCol, Modifier.size(14.dp))
        Spacer(Modifier.width(ClipmoTheme.spacing.xs))
        BasicText(
            label,
            style = ClipmoTheme.typography.labelSmall.copy(
                fontWeight = if (selected) FontWeight.Bold else FontWeight.Medium,
                color = textCol,
            ),
        )
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
    onRemoveFromCollection: (() -> Unit)? = null,
) {
    val colors = ClipmoTheme.colors
    val space = ClipmoTheme.spacing
    val type = ClipmoTheme.typography

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

    val timeLabel = remember(clip.timestamp) { ClipmoTimeFormat.short.format(Date(clip.timestamp)) }

    Row(
        Modifier
            .padding(horizontal = space.md, vertical = 3.dp)
            .fillMaxWidth()
            .heightIn(min = ClipmoTheme.dimensions.clipMinHeight)
            .clip(RoundedCornerShape(ClipmoTheme.shapes.card))
            .background(if (selected) colors.surfaceContainerHighest else colors.surfaceContainerLow)
            .border(
                1.dp,
                if (selected) colors.accent else colors.borderSubtle,
                RoundedCornerShape(ClipmoTheme.shapes.card),
            )
            .combinedClickable(
                onClick = {
                    if (selectionMode && onSelect != null) onSelect() else onOpen?.invoke(clip) ?: onCopy(clip)
                },
                onLongClick = onSelect ?: { onDelete(clip) },
            )
            .padding(start = space.sm, end = space.xs, top = space.sm, bottom = space.sm),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // Leading Thumbnail / Icon Box
        Box(
            Modifier
                .size(ClipmoTheme.dimensions.thumbnail)
                .clip(RoundedCornerShape(10.dp))
                .background(
                    when {
                        clip.kind == ClipKind.URL -> colors.secondaryContainer
                        clip.kind == ClipKind.IMAGE -> colors.surfaceContainer
                        else -> colors.surfaceContainer
                    },
                ),
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
                else -> ClipmoIcon(
                    iconFor(clip.kind),
                    if (clip.kind == ClipKind.URL) colors.secondary else colors.textSecondary,
                    Modifier.size(18.dp),
                )
            }
        }

        Spacer(Modifier.width(space.sm))

        // Content & Metadata
        Column(Modifier.weight(1f)) {
            BasicText(
                clipPreview(clip),
                style = when (clip.kind) {
                    ClipKind.URL -> type.bodyMedium.copy(color = colors.accent, fontWeight = FontWeight.Medium)
                    else -> type.bodyMedium.copy(color = colors.textPrimary)
                },
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            Spacer(Modifier.height(space.xxs))
            Row(verticalAlignment = Alignment.CenterVertically) {
                Box(
                    Modifier
                        .clip(RoundedCornerShape(4.dp))
                        .background(colors.surfaceContainer)
                        .padding(horizontal = 5.dp, vertical = 1.dp),
                ) {
                    BasicText(
                        kindLabel(clip.kind),
                        style = type.metadata.copy(color = colors.textSecondary, fontWeight = FontWeight.SemiBold, fontSize = 10.sp),
                    )
                }
                Spacer(Modifier.width(space.xs))
                BasicText(
                    timeLabel,
                    style = type.metadata.copy(color = colors.textMuted),
                )
            }
        }

        // Action Icons
        if (!selectionMode) {
            if (onRemoveFromCollection != null) {
                ClipmoIconButton(
                    kind = ClipmoIconKind.CLOSE,
                    description = "Remove from collection",
                    onClick = onRemoveFromCollection,
                    tint = colors.textMuted,
                )
            }
            ClipmoIconButton(
                kind = if (clip.favorite) ClipmoIconKind.STAR_FILLED else ClipmoIconKind.STAR,
                description = if (clip.favorite) "Unstar" else "Star",
                onClick = { onFavorite(clip) },
                tint = if (clip.favorite) colors.warning else colors.textMuted,
            )
            ClipmoIconButton(
                kind = ClipmoIconKind.COPY,
                description = "Copy",
                onClick = { onCopy(clip) },
                tint = colors.textSecondary,
            )
        }
    }
}

// -----------------------------------------------------------------------------
// Collection Card
// -----------------------------------------------------------------------------

@Composable
private fun ClipmoCollectionCard(
    name: String,
    icon: ClipmoIconKind,
    count: Int,
    selected: Boolean,
    modifier: Modifier = Modifier,
    onDelete: (() -> Unit)? = null,
    onClick: () -> Unit,
) {
    val colors = ClipmoTheme.colors
    val space = ClipmoTheme.spacing
    val type = ClipmoTheme.typography

    val bg by animateColorAsState(
        if (selected) colors.surfaceContainerHighest else colors.surfaceContainerLow,
        tween(150),
        label = "collBg",
    )
    val borderCol by animateColorAsState(
        if (selected) colors.accent else colors.border,
        tween(150),
        label = "collBorder",
    )

    Column(
        modifier
            .height(104.dp)
            .clip(RoundedCornerShape(ClipmoTheme.shapes.card))
            .background(bg)
            .border(1.dp, borderCol, RoundedCornerShape(ClipmoTheme.shapes.card))
            .combinedClickable(
                interactionSource = remember { MutableInteractionSource() },
                indication = null,
                onClick = onClick,
                onLongClick = onDelete,
            )
            .padding(space.md),
        verticalArrangement = Arrangement.SpaceBetween,
    ) {
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(
                Modifier
                    .size(32.dp)
                    .clip(RoundedCornerShape(8.dp))
                    .background(if (selected) colors.accentMuted else colors.surfaceContainer),
                contentAlignment = Alignment.Center,
            ) {
                ClipmoIcon(icon, if (selected) colors.accent else colors.textSecondary, Modifier.size(16.dp))
            }
            if (onDelete != null) {
                Box(
                    Modifier
                        .size(32.dp)
                        .clip(CircleShape)
                        .clickable(
                            interactionSource = remember { MutableInteractionSource() },
                            indication = null,
                            onClick = onDelete,
                        ),
                    contentAlignment = Alignment.Center,
                ) {
                    ClipmoIcon(
                        ClipmoIconKind.DELETE,
                        if (selected) colors.danger else colors.textMuted,
                        Modifier.size(15.dp),
                    )
                }
            } else if (selected) {
                Box(
                    Modifier
                        .size(8.dp)
                        .clip(CircleShape)
                        .background(colors.accent),
                )
            }
        }

        Column {
            BasicText(
                name,
                style = type.label.copy(color = colors.textPrimary),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            BasicText(
                "$count ${if (count == 1) "clip" else "clips"}",
                style = type.metadata.copy(color = colors.textMuted),
            )
        }
    }
}

// -----------------------------------------------------------------------------
// Device Card
// -----------------------------------------------------------------------------

@Composable
private fun ClipmoDeviceCard(
    name: String,
    platformLabel: String,
    statusLabel: String,
    online: Boolean,
    isLocal: Boolean = false,
    action: String? = null,
    onAction: (() -> Unit)? = null,
) {
    val colors = ClipmoTheme.colors
    val space = ClipmoTheme.spacing
    val type = ClipmoTheme.typography

    Row(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(ClipmoTheme.shapes.card))
            .background(colors.surfaceContainerLow)
            .border(1.dp, colors.border, RoundedCornerShape(ClipmoTheme.shapes.card))
            .padding(space.md),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            Modifier
                .size(44.dp)
                .clip(RoundedCornerShape(12.dp))
                .background(if (online) colors.successContainer else colors.surfaceContainer),
            contentAlignment = Alignment.Center,
        ) {
            ClipmoIcon(
                kind = if (isLocal) ClipmoIconKind.PHONE else ClipmoIconKind.DESKTOP,
                color = if (online) colors.onSuccessContainer else colors.textMuted,
                modifier = Modifier.size(22.dp),
            )
        }

        Spacer(Modifier.width(space.md))

        Column(Modifier.weight(1f)) {
            BasicText(
                name,
                style = type.titleSmall.copy(color = colors.textPrimary),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Spacer(Modifier.height(space.xxs))
            Row(verticalAlignment = Alignment.CenterVertically) {
                Box(
                    Modifier
                        .size(7.dp)
                        .clip(CircleShape)
                        .background(if (online) colors.success else colors.textMuted),
                )
                Spacer(Modifier.width(space.xs))
                BasicText(
                    "$platformLabel · $statusLabel",
                    style = type.metadata.copy(
                        color = if (online) colors.textSecondary else colors.textMuted,
                    ),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }

        if (action != null && onAction != null) {
            Spacer(Modifier.width(space.sm))
            ClipmoButton(
                label = action,
                style = ClipmoButtonStyle.GHOST,
                onClick = onAction,
            )
        }
    }
}

// -----------------------------------------------------------------------------
// Settings Group and Rows
// -----------------------------------------------------------------------------

@Composable
private fun ClipmoSettingsGroup(title: String, content: @Composable () -> Unit) {
    val colors = ClipmoTheme.colors
    val space = ClipmoTheme.spacing

    BasicText(
        title,
        style = ClipmoTheme.typography.section.copy(color = colors.textSecondary),
    )
    Spacer(Modifier.height(space.xs))
    Column(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(ClipmoTheme.shapes.panel))
            .background(colors.surfaceContainerLow)
            .border(1.dp, colors.border, RoundedCornerShape(ClipmoTheme.shapes.panel))
            .padding(space.md),
    ) {
        content()
    }
}

@Composable
private fun ClipmoSettingsToggle(title: String, subtitle: String, checked: Boolean, onChanged: (Boolean) -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 52.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            BasicText(
                title,
                style = ClipmoTheme.typography.bodyMedium.copy(color = ClipmoTheme.colors.textPrimary, fontWeight = FontWeight.Medium),
            )
            Spacer(Modifier.height(ClipmoTheme.spacing.xxs))
            BasicText(
                subtitle,
                style = ClipmoTheme.typography.metadata.copy(color = ClipmoTheme.colors.textMuted),
            )
        }
        Spacer(Modifier.width(ClipmoTheme.spacing.md))
        ClipmoToggle(checked, onChanged)
    }
}

@Composable
private fun ClipmoToggle(checked: Boolean, onChanged: (Boolean) -> Unit) {
    val colors = ClipmoTheme.colors
    val track by animateColorAsState(
        if (checked) colors.accent else colors.surfaceContainerHighest,
        tween(150),
        label = "toggleTrack",
    )
    val thumbPos by animateFloatAsState(
        if (checked) 22f else 3f,
        tween(150),
        label = "toggleThumb",
    )

    Box(
        Modifier
            .width(48.dp)
            .height(28.dp)
            .clip(RoundedCornerShape(50))
            .background(track)
            .semantics { role = Role.Switch; contentDescription = if (checked) "On" else "Off" }
            .combinedClickable(onClick = { onChanged(!checked) }),
    ) {
        Box(
            Modifier
                .padding(start = thumbPos.dp, top = 3.dp)
                .size(22.dp)
                .clip(CircleShape)
                .background(if (checked) Color.White else colors.textSecondary),
        )
    }
}

@Composable
private fun ClipmoAppearanceSelector(
    currentMode: ClipmoThemeMode,
    onModeSelected: (ClipmoThemeMode) -> Unit,
) {
    val colors = ClipmoTheme.colors

    Row(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(ClipmoTheme.shapes.sm))
            .background(colors.surfaceContainer)
            .padding(4.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        ClipmoThemeMode.entries.forEach { mode ->
            val selected = currentMode == mode
            val bg by animateColorAsState(
                if (selected) colors.surface else Color.Transparent,
                tween(150),
                label = "appModeBg",
            )
            val textCol by animateColorAsState(
                if (selected) colors.textPrimary else colors.textMuted,
                tween(150),
                label = "appModeText",
            )

            Box(
                Modifier
                    .weight(1f)
                    .height(36.dp)
                    .clip(RoundedCornerShape(ClipmoTheme.shapes.xs))
                    .background(bg)
                    .combinedClickable(onClick = { onModeSelected(mode) }),
                contentAlignment = Alignment.Center,
            ) {
                BasicText(
                    text = mode.name.lowercase().replaceFirstChar { it.uppercase() },
                    style = ClipmoTheme.typography.labelSmall.copy(
                        color = textCol,
                        fontWeight = if (selected) FontWeight.Bold else FontWeight.Normal,
                    ),
                )
            }
        }
    }
}

@Composable
private fun ClipmoActionRow(title: String, subtitle: String, color: Color, onClick: () -> Unit) {
    val space = ClipmoTheme.spacing
    Row(
        Modifier
            .fillMaxWidth()
            .combinedClickable(onClick = onClick)
            .padding(vertical = space.xs),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        ClipmoIcon(ClipmoIconKind.DELETE, color, Modifier.size(18.dp))
        Spacer(Modifier.width(space.sm))
        Column {
            BasicText(title, style = ClipmoTheme.typography.bodyMedium.copy(color = color, fontWeight = FontWeight.SemiBold))
            BasicText(subtitle, style = ClipmoTheme.typography.metadata.copy(color = ClipmoTheme.colors.textMuted))
        }
    }
}

// -----------------------------------------------------------------------------
// Bottom Navigation Bar
// -----------------------------------------------------------------------------

@Composable
private fun ClipmoBottomBar(selected: ClipmoScreen, onSelected: (ClipmoScreen) -> Unit) {
    val colors = ClipmoTheme.colors
    val space = ClipmoTheme.spacing

    Row(
        Modifier
            .fillMaxWidth()
            .height(ClipmoTheme.dimensions.navBarHeight)
            .background(colors.surface)
            .border(1.dp, colors.borderSubtle)
            .padding(horizontal = space.sm),
        horizontalArrangement = Arrangement.SpaceAround,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        listOf(
            Triple(ClipmoScreen.HISTORY, ClipmoIconKind.CLIPBOARD, "Clips"),
            Triple(ClipmoScreen.COLLECTIONS, ClipmoIconKind.COLLECTION, "Collections"),
            Triple(ClipmoScreen.DEVICES, ClipmoIconKind.DEVICE, "Devices"),
        ).forEach { (screen, icon, label) ->
            val active = selected == screen
            val pillWidth by animateFloatAsState(if (active) 60f else 36f, tween(200), label = "navPillW")
            val iconCol by animateColorAsState(if (active) colors.accent else colors.textMuted, tween(150), label = "navIcon")
            val textCol by animateColorAsState(if (active) colors.textPrimary else colors.textMuted, tween(150), label = "navText")

            Column(
                Modifier
                    .weight(1f)
                    .fillMaxHeight()
                    .combinedClickable(
                        interactionSource = remember { MutableInteractionSource() },
                        indication = null,
                        onClick = { onSelected(screen) },
                    ),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center,
            ) {
                Box(
                    Modifier
                        .size(width = pillWidth.dp, height = 30.dp)
                        .clip(RoundedCornerShape(15.dp))
                        .background(if (active) colors.accentMuted else Color.Transparent),
                    contentAlignment = Alignment.Center,
                ) {
                    ClipmoIcon(icon, iconCol, Modifier.size(18.dp))
                }
                Spacer(Modifier.height(2.dp))
                BasicText(
                    label,
                    style = ClipmoTheme.typography.metadata.copy(
                        color = textCol,
                        fontWeight = if (active) FontWeight.Bold else FontWeight.Normal,
                    ),
                )
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Button & Icon Button Styles
// -----------------------------------------------------------------------------

private enum class ClipmoButtonStyle { PRIMARY, SECONDARY, DANGER, TONAL_DANGER, GHOST }

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
    val space = ClipmoTheme.spacing
    val shape = RoundedCornerShape(ClipmoTheme.shapes.pill)

    val background = when (style) {
        ClipmoButtonStyle.PRIMARY -> colors.accent
        ClipmoButtonStyle.SECONDARY -> colors.surfaceContainerHigh
        ClipmoButtonStyle.DANGER -> colors.danger
        ClipmoButtonStyle.TONAL_DANGER -> colors.dangerContainer
        ClipmoButtonStyle.GHOST -> colors.surfaceContainerLow
    }
    val content = tint ?: when (style) {
        ClipmoButtonStyle.PRIMARY -> colors.onAccent
        ClipmoButtonStyle.SECONDARY -> colors.textPrimary
        ClipmoButtonStyle.DANGER -> Color.White
        ClipmoButtonStyle.TONAL_DANGER -> colors.danger
        ClipmoButtonStyle.GHOST -> colors.textSecondary
    }
    val borderCol = when (style) {
        ClipmoButtonStyle.GHOST -> colors.border
        else -> Color.Transparent
    }

    Row(
        modifier
            .height(42.dp)
            .clip(shape)
            .background(background)
            .border(1.dp, borderCol, shape)
            .clickable(
                interactionSource = remember { MutableInteractionSource() },
                indication = null,
                onClick = onClick,
            )
            .padding(horizontal = space.sm),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) {
            ClipmoIcon(icon, content, Modifier.size(16.dp))
            Spacer(Modifier.width(space.xs))
        }
        BasicText(
            label,
            style = ClipmoTheme.typography.label.copy(fontWeight = FontWeight.SemiBold, color = content),
            maxLines = 1,
        )
    }
}

@Composable
private fun ClipmoIconButton(
    kind: ClipmoIconKind,
    description: String,
    onClick: () -> Unit,
    tint: Color = ClipmoTheme.colors.textSecondary,
) {
    val interaction = remember { MutableInteractionSource() }
    Box(
        Modifier
            .requiredSize(ClipmoTheme.dimensions.touch)
            .clip(CircleShape)
            .clickable(interactionSource = interaction, indication = null, onClick = onClick)
            .semantics { contentDescription = description },
        contentAlignment = Alignment.Center,
    ) {
        ClipmoIcon(kind, tint, Modifier.size(ClipmoTheme.dimensions.icon))
    }
}

// -----------------------------------------------------------------------------
// Floating Copied Pill
// -----------------------------------------------------------------------------

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
        modifier.padding(bottom = 76.dp),
        enter = fadeIn(tween(100)) + scaleIn(initialScale = 0.85f, animationSpec = spring(dampingRatio = 0.7f)),
        exit = fadeOut(tween(180)) + scaleOut(targetScale = 0.9f, animationSpec = tween(180)),
    ) {
        Row(
            Modifier
                .clip(RoundedCornerShape(ClipmoTheme.shapes.pill))
                .background(colors.surfaceContainerHighest)
                .border(1.dp, colors.border, RoundedCornerShape(ClipmoTheme.shapes.pill))
                .padding(horizontal = ClipmoTheme.spacing.lg, vertical = ClipmoTheme.spacing.sm),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(
                Modifier
                    .size(20.dp)
                    .clip(CircleShape)
                    .background(colors.successContainer),
                contentAlignment = Alignment.Center,
            ) {
                ClipmoIcon(ClipmoIconKind.CHECK, colors.onSuccessContainer, Modifier.size(13.dp))
            }
            Spacer(Modifier.width(ClipmoTheme.spacing.xs))
            BasicText(
                "Copied to clipboard",
                style = ClipmoTheme.typography.label.copy(
                    fontWeight = FontWeight.Bold,
                    color = colors.textPrimary,
                ),
            )
        }
    }
}

@Composable
private fun ClipmoDeviceHeader(name: String, count: Int) {
    val colors = ClipmoTheme.colors
    val space = ClipmoTheme.spacing

    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = space.md, vertical = space.xs),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        ClipmoIcon(ClipmoIconKind.DEVICE, colors.textMuted, Modifier.size(13.dp))
        Spacer(Modifier.width(space.xs))
        BasicText(
            name,
            style = ClipmoTheme.typography.section.copy(color = colors.textSecondary),
            modifier = Modifier.weight(1f),
        )
        Box(
            Modifier
                .clip(RoundedCornerShape(4.dp))
                .background(colors.surfaceContainer)
                .padding(horizontal = 6.dp, vertical = 2.dp),
        ) {
            BasicText(
                count.toString(),
                style = ClipmoTheme.typography.metadata.copy(color = colors.textMuted, fontWeight = FontWeight.Bold),
            )
        }
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
    val space = ClipmoTheme.spacing

    Row(
        Modifier
            .fillMaxWidth()
            .height(ClipmoTheme.dimensions.searchHeight)
            .clip(RoundedCornerShape(ClipmoTheme.shapes.search))
            .background(colors.surfaceContainerHighest)
            .border(1.dp, colors.accent, RoundedCornerShape(ClipmoTheme.shapes.search))
            .padding(start = space.md, end = space.xs),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        BasicText(
            "$selectedCount selected",
            style = ClipmoTheme.typography.bodyMedium.copy(color = colors.textPrimary, fontWeight = FontWeight.Bold),
            modifier = Modifier.weight(1f),
        )
        ClipmoIconButton(ClipmoIconKind.COLLECTION, "Add selected to collection", onCollection, colors.accent)
        ClipmoIconButton(ClipmoIconKind.STAR, "Star selected", onStar, colors.warning)
        ClipmoIconButton(ClipmoIconKind.DELETE, "Delete selected", onDelete, colors.danger)
        ClipmoIconButton(ClipmoIconKind.CLOSE, "Exit selection", onClose, colors.textSecondary)
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

    Box(Modifier.fillMaxWidth()) {
        if (offsetX != 0f) {
            val movingToCollection = offsetX < 0
            Box(
                Modifier
                    .matchParentSize()
                    .padding(horizontal = ClipmoTheme.spacing.md, vertical = 3.dp)
                    .clip(RoundedCornerShape(ClipmoTheme.shapes.card))
                    .background(if (movingToCollection) colors.accent else colors.danger)
                    .padding(horizontal = ClipmoTheme.spacing.md),
                contentAlignment = if (movingToCollection) Alignment.CenterEnd else Alignment.CenterStart,
            ) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    ClipmoIcon(
                        if (movingToCollection) ClipmoIconKind.COLLECTION else ClipmoIconKind.DELETE,
                        Color.White,
                        Modifier.size(18.dp),
                    )
                    Spacer(Modifier.width(ClipmoTheme.spacing.sm))
                    BasicText(
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
private fun ClipmoEmptyState(title: String, message: String) {
    val space = ClipmoTheme.spacing
    Column(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 32.dp, vertical = 48.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Image(
            painter = painterResource(R.drawable.clipmo_logo),
            contentDescription = null,
            modifier = Modifier
                .size(56.dp)
                .clip(RoundedCornerShape(14.dp)),
        )
        Spacer(Modifier.height(space.md))
        BasicText(
            title,
            style = ClipmoTheme.typography.title.copy(color = ClipmoTheme.colors.textPrimary),
        )
        Spacer(Modifier.height(space.xs))
        BasicText(
            message,
            style = ClipmoTheme.typography.bodyMedium.copy(color = ClipmoTheme.colors.textMuted),
        )
    }
}

// -----------------------------------------------------------------------------
// Overlays & Sheets
// -----------------------------------------------------------------------------

@Composable
private fun ClipmoCreateCollectionOverlay(
    onDismiss: () -> Unit,
    onCreate: (String) -> Unit,
) {
    var name by remember { mutableStateOf("") }
    val colors = ClipmoTheme.colors
    val space = ClipmoTheme.spacing

    Box(
        Modifier
            .fillMaxSize()
            .background(colors.scrim)
            .combinedClickable(
                interactionSource = remember { MutableInteractionSource() },
                indication = null,
                onClick = onDismiss,
            ),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            Modifier
                .widthIn(max = 340.dp)
                .padding(space.xl)
                .clip(RoundedCornerShape(ClipmoTheme.shapes.panel))
                .background(colors.surfaceContainerLow)
                .border(1.dp, colors.border, RoundedCornerShape(ClipmoTheme.shapes.panel))
                .combinedClickable(
                    interactionSource = remember { MutableInteractionSource() },
                    indication = null,
                    onClick = {},
                )
                .padding(space.lg),
        ) {
            BasicText("New collection", style = ClipmoTheme.typography.title.copy(color = colors.textPrimary))
            Spacer(Modifier.height(space.md))
            ClipmoSearchBar(name, "Collection name", { name = it.take(40) }, searchIcon = false)
            Spacer(Modifier.height(space.lg))
            Row(horizontalArrangement = Arrangement.spacedBy(space.sm)) {
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
    val space = ClipmoTheme.spacing

    Box(
        Modifier
            .fillMaxSize()
            .background(colors.scrim)
            .combinedClickable(
                interactionSource = remember { MutableInteractionSource() },
                indication = null,
                onClick = onDismiss,
            ),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            Modifier
                .widthIn(max = 340.dp)
                .padding(space.xl)
                .clip(RoundedCornerShape(ClipmoTheme.shapes.panel))
                .background(colors.surfaceContainerLow)
                .border(1.dp, colors.border, RoundedCornerShape(ClipmoTheme.shapes.panel))
                .combinedClickable(
                    interactionSource = remember { MutableInteractionSource() },
                    indication = null,
                    onClick = {},
                )
                .padding(space.lg),
        ) {
            BasicText("Add to collection", style = ClipmoTheme.typography.title.copy(color = colors.textPrimary))
            Spacer(Modifier.height(space.xxs))
            BasicText(
                if (selectedCount == 1) "Choose a collection for this clip" else "Choose a collection for $selectedCount clips",
                style = ClipmoTheme.typography.metadata.copy(color = colors.textMuted),
            )
            Spacer(Modifier.height(space.md))
            if (collections.isEmpty()) {
                BasicText(
                    "Create a collection from the Collections tab first.",
                    style = ClipmoTheme.typography.body.copy(color = colors.textSecondary),
                )
            } else {
                LazyColumn(
                    Modifier.heightIn(max = 280.dp),
                    verticalArrangement = Arrangement.spacedBy(space.xs),
                ) {
                    items(collections, key = { it }) { collection ->
                        Row(
                            Modifier
                                .fillMaxWidth()
                                .clip(RoundedCornerShape(ClipmoTheme.shapes.card))
                                .background(colors.surfaceContainer)
                                .combinedClickable(onClick = { onCollection(collection) })
                                .padding(space.md),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            ClipmoIcon(ClipmoIconKind.COLLECTION, colors.accent, Modifier.size(17.dp))
                            Spacer(Modifier.width(space.sm))
                            BasicText(collection, style = ClipmoTheme.typography.body.copy(color = colors.textPrimary))
                        }
                    }
                }
            }
            Spacer(Modifier.height(space.md))
            ClipmoButton("Close", ClipmoButtonStyle.GHOST, Modifier.fillMaxWidth(), onClick = onDismiss)
        }
    }
}

// -----------------------------------------------------------------------------
// Clip Detail / Edit Bottom Sheet
// -----------------------------------------------------------------------------

@Composable
private fun ClipmoDetailSheet(
    clip: ClipRecord,
    collections: List<String>,
    onCopy: (ClipRecord) -> Unit,
    onFavorite: (ClipRecord) -> Unit = {},
    onDelete: (ClipRecord) -> Unit,
    onEdit: (String) -> Unit,
    onAddToCollection: (Set<Long>, String) -> Unit = { _, _ -> },
    onCreateCollection: (String) -> Unit = {},
    onRemoveFromCollection: (Set<Long>, String) -> Unit = { _, _ -> },
    onDismiss: () -> Unit,
) {
    val colors = ClipmoTheme.colors
    val space = ClipmoTheme.spacing
    val type = ClipmoTheme.typography
    val editable = clip.kind == ClipKind.TEXT || clip.kind == ClipKind.URL
    var editing by remember { mutableStateOf(false) }
    var draft by remember(clip.id) { mutableStateOf(clip.content) }
    var isFavorite by remember(clip.id, clip.favorite) { mutableStateOf(clip.favorite) }

    LaunchedEffect(clip.favorite) {
        isFavorite = clip.favorite
    }

    BackHandler {
        if (editing) {
            editing = false
            draft = clip.content
        } else onDismiss()
    }

    val previewPath = remember(clip.assetPaths, clip.kind, clip.content) {
        if (clip.kind == ClipKind.IMAGE) {
            val assets = clip.assetPaths?.lineSequence()?.filter(String::isNotBlank)?.toList().orEmpty()
            assets.firstOrNull { !it.endsWith("thumb.jpg", ignoreCase = true) }
                ?: assets.firstOrNull()
                ?: clip.content.takeIf { it.isNotBlank() && it.startsWith("/") }
        } else null
    }

    val imageBitmap by produceState<androidx.compose.ui.graphics.ImageBitmap?>(null, previewPath) {
        val path = previewPath ?: return@produceState
        val decoded = withContext(Dispatchers.IO) {
            runCatching {
                val opts = android.graphics.BitmapFactory.Options().apply { inJustDecodeBounds = true }
                android.graphics.BitmapFactory.decodeFile(path, opts)
                val maxDim = maxOf(opts.outWidth, opts.outHeight)
                val sample = if (maxDim > 2048) (maxDim / 1080).coerceAtLeast(1) else 1
                val decodeOpts = android.graphics.BitmapFactory.Options().apply {
                    inSampleSize = sample
                    inPreferredConfig = android.graphics.Bitmap.Config.ARGB_8888
                }
                android.graphics.BitmapFactory.decodeFile(path, decodeOpts)
            }.getOrNull()
        }
        if (decoded != null) {
            value = decoded.asImageBitmap()
        }
    }

    Column(
        Modifier
            .fillMaxWidth()
            .wrapContentHeight()
            .clip(RoundedCornerShape(topStart = ClipmoTheme.shapes.sheet, topEnd = ClipmoTheme.shapes.sheet))
            .background(colors.surface)
            .border(1.dp, colors.border, RoundedCornerShape(topStart = ClipmoTheme.shapes.sheet, topEnd = ClipmoTheme.shapes.sheet))
            .combinedClickable(
                interactionSource = remember { MutableInteractionSource() },
                indication = null,
                onClick = {},
            )
            .windowInsetsPadding(WindowInsets.navigationBars)
            .windowInsetsPadding(WindowInsets.ime)
            .padding(bottom = space.md),
    ) {
        // Drag handle
        Box(
            Modifier
                .align(Alignment.CenterHorizontally)
                .padding(top = space.sm)
                .size(width = 36.dp, height = 4.dp)
                .clip(CircleShape)
                .background(colors.border),
        )

        // Sheet Header
        Row(
            Modifier
                .fillMaxWidth()
                .padding(horizontal = space.lg, vertical = space.sm),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(
                Modifier
                    .size(36.dp)
                    .clip(RoundedCornerShape(10.dp))
                    .background(colors.accentMuted),
                contentAlignment = Alignment.Center,
            ) {
                ClipmoIcon(iconFor(clip.kind), colors.onAccentMuted, Modifier.size(18.dp))
            }
            Spacer(Modifier.width(space.md))
            Column(Modifier.weight(1f)) {
                BasicText(kindLabel(clip.kind), style = type.title.copy(color = colors.textPrimary))
                BasicText(
                    ClipmoTimeFormat.short.format(Date(clip.timestamp)),
                    style = type.metadata.copy(color = colors.textMuted),
                )
            }
            ClipmoIconButton(
                kind = if (isFavorite) ClipmoIconKind.STAR_FILLED else ClipmoIconKind.STAR,
                description = "Favorite",
                onClick = {
                    isFavorite = !isFavorite
                    onFavorite(clip.copy(favorite = isFavorite))
                },
                tint = if (isFavorite) colors.warning else colors.textMuted,
            )
            ClipmoIconButton(
                kind = ClipmoIconKind.CLOSE,
                description = "Close",
                onClick = onDismiss,
                tint = colors.textMuted,
            )
        }

        // Content Viewer / Editor Surface
        Box(
            Modifier
                .fillMaxWidth()
                .heightIn(min = if (clip.kind == ClipKind.IMAGE) 220.dp else 90.dp, max = 400.dp)
                .padding(horizontal = space.lg)
                .clip(RoundedCornerShape(ClipmoTheme.shapes.card))
                .background(colors.surfaceContainerLow)
                .border(1.dp, colors.borderSubtle, RoundedCornerShape(ClipmoTheme.shapes.card)),
        ) {
            if (editing) {
                BasicTextField(
                    value = draft,
                    onValueChange = { draft = it },
                    textStyle = type.body.copy(color = colors.textPrimary),
                    cursorBrush = SolidColor(colors.accent),
                    modifier = Modifier
                        .fillMaxWidth()
                        .heightIn(min = 160.dp, max = 340.dp)
                        .padding(space.md)
                        .verticalScroll(rememberScrollState()),
                )
            } else if (clip.kind == ClipKind.IMAGE) {
                Box(
                    Modifier
                        .fillMaxWidth()
                        .heightIn(min = 220.dp, max = 400.dp)
                        .padding(space.xs),
                    contentAlignment = Alignment.Center,
                ) {
                    val bitmap = imageBitmap
                    if (bitmap != null) {
                        Image(
                            bitmap = bitmap,
                            contentDescription = "Image preview",
                            contentScale = ContentScale.Fit,
                            modifier = Modifier
                                .fillMaxWidth()
                                .heightIn(min = 200.dp, max = 380.dp)
                                .clip(RoundedCornerShape(12.dp)),
                        )
                    } else {
                        BasicText(
                            "Loading image...",
                            style = type.bodyMedium.copy(color = colors.textMuted),
                        )
                    }
                }
            } else {
                Column(
                    Modifier
                        .fillMaxWidth()
                        .heightIn(min = 70.dp, max = 340.dp)
                        .padding(space.md)
                        .verticalScroll(rememberScrollState()),
                ) {
                    BasicText(
                        clip.content,
                        style = type.body.copy(color = colors.textPrimary),
                    )
                }
            }
        }

        Spacer(Modifier.height(space.sm))

        // Collection Tags Row (Removable pills, available collections, and inline create)
        val availableCollections = remember(collections, clip.tags) {
            collections.filter { it !in clip.tags }
        }
        var creatingNewCollection by remember { mutableStateOf(false) }
        var newCollectionName by remember { mutableStateOf("") }

        Row(
            Modifier
                .fillMaxWidth()
                .padding(horizontal = space.lg)
                .horizontalScroll(rememberScrollState()),
            horizontalArrangement = Arrangement.spacedBy(space.xs),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            // Active attached tags
            clip.tags.forEach { tag ->
                Row(
                    Modifier
                        .clip(RoundedCornerShape(ClipmoTheme.shapes.pill))
                        .background(colors.accentMuted)
                        .border(1.dp, colors.accent.copy(alpha = 0.35f), RoundedCornerShape(ClipmoTheme.shapes.pill))
                        .padding(start = space.sm, end = space.xxs, top = space.xxs, bottom = space.xxs),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    ClipmoIcon(ClipmoIconKind.COLLECTION, colors.accent, Modifier.size(13.dp))
                    Spacer(Modifier.width(space.xxs))
                    BasicText(
                        tag,
                        style = type.labelSmall.copy(color = colors.textPrimary, fontWeight = FontWeight.SemiBold),
                    )
                    Spacer(Modifier.width(space.xxs))
                    Box(
                        Modifier
                            .size(22.dp)
                            .clip(CircleShape)
                            .clickable(
                                interactionSource = remember { MutableInteractionSource() },
                                indication = null,
                                onClick = { onRemoveFromCollection(setOf(clip.id), tag) },
                            ),
                        contentAlignment = Alignment.Center,
                    ) {
                        ClipmoIcon(ClipmoIconKind.CLOSE, colors.textMuted, Modifier.size(11.dp))
                    }
                }
            }

            // Quick add available collections
            availableCollections.forEach { tag ->
                Row(
                    Modifier
                        .clip(RoundedCornerShape(ClipmoTheme.shapes.pill))
                        .background(colors.surfaceContainerHigh)
                        .border(1.dp, colors.borderSubtle, RoundedCornerShape(ClipmoTheme.shapes.pill))
                        .clickable(
                            interactionSource = remember { MutableInteractionSource() },
                            indication = null,
                            onClick = { onAddToCollection(setOf(clip.id), tag) },
                        )
                        .padding(horizontal = space.sm, vertical = space.xs),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    ClipmoIcon(ClipmoIconKind.PLUS, colors.textSecondary, Modifier.size(12.dp))
                    Spacer(Modifier.width(space.xxs))
                    BasicText(
                        tag,
                        style = type.labelSmall.copy(color = colors.textSecondary, fontWeight = FontWeight.Medium),
                    )
                }
            }

            // Inline create new collection
            if (creatingNewCollection) {
                Row(
                    Modifier
                        .clip(RoundedCornerShape(ClipmoTheme.shapes.pill))
                        .background(colors.surfaceContainerLow)
                        .border(1.dp, colors.accent, RoundedCornerShape(ClipmoTheme.shapes.pill))
                        .padding(start = space.sm, end = space.xxs, top = space.xxs, bottom = space.xxs),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    BasicTextField(
                        value = newCollectionName,
                        onValueChange = { newCollectionName = it },
                        singleLine = true,
                        textStyle = type.labelSmall.copy(color = colors.textPrimary),
                        cursorBrush = SolidColor(colors.accent),
                        modifier = Modifier.widthIn(min = 60.dp, max = 110.dp),
                        decorationBox = { inner ->
                            if (newCollectionName.isEmpty()) {
                                BasicText("Name...", style = type.labelSmall.copy(color = colors.textMuted))
                            }
                            inner()
                        },
                    )
                    Spacer(Modifier.width(space.xxs))
                    Box(
                        Modifier
                            .size(22.dp)
                            .clip(CircleShape)
                            .clickable(
                                interactionSource = remember { MutableInteractionSource() },
                                indication = null,
                                onClick = {
                                    if (newCollectionName.isNotBlank()) {
                                        val name = newCollectionName.trim()
                                        onCreateCollection(name)
                                        onAddToCollection(setOf(clip.id), name)
                                        newCollectionName = ""
                                        creatingNewCollection = false
                                    }
                                },
                            ),
                        contentAlignment = Alignment.Center,
                    ) {
                        ClipmoIcon(ClipmoIconKind.CHECK, colors.accent, Modifier.size(13.dp))
                    }
                    Box(
                        Modifier
                            .size(22.dp)
                            .clip(CircleShape)
                            .clickable(
                                interactionSource = remember { MutableInteractionSource() },
                                indication = null,
                                onClick = {
                                    newCollectionName = ""
                                    creatingNewCollection = false
                                },
                            ),
                        contentAlignment = Alignment.Center,
                    ) {
                        ClipmoIcon(ClipmoIconKind.CLOSE, colors.textMuted, Modifier.size(11.dp))
                    }
                }
            } else {
                Row(
                    Modifier
                        .clip(RoundedCornerShape(ClipmoTheme.shapes.pill))
                        .background(colors.surfaceContainerLow)
                        .border(1.dp, colors.borderSubtle, RoundedCornerShape(ClipmoTheme.shapes.pill))
                        .clickable(
                            interactionSource = remember { MutableInteractionSource() },
                            indication = null,
                            onClick = { creatingNewCollection = true },
                        )
                        .padding(horizontal = space.sm, vertical = space.xs),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    ClipmoIcon(ClipmoIconKind.PLUS, colors.accent, Modifier.size(12.dp))
                    Spacer(Modifier.width(space.xxs))
                    BasicText(
                        if (clip.tags.isEmpty() && availableCollections.isEmpty()) "Add to collection" else "New",
                        style = type.labelSmall.copy(color = colors.accent, fontWeight = FontWeight.SemiBold),
                    )
                }
            }
        }

        Spacer(Modifier.height(space.sm))

        // Action Bar (Thumb Reach Zone - Single Clean Row)
        Row(
            Modifier
                .fillMaxWidth()
                .padding(horizontal = space.lg),
            horizontalArrangement = Arrangement.spacedBy(space.sm),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (editing) {
                ClipmoButton(
                    label = "Cancel",
                    style = ClipmoButtonStyle.GHOST,
                    modifier = Modifier.weight(1f),
                ) {
                    editing = false
                    draft = clip.content
                }
                ClipmoButton(
                    label = "Save",
                    style = ClipmoButtonStyle.PRIMARY,
                    modifier = Modifier.weight(1f),
                    icon = ClipmoIconKind.CHECK,
                ) {
                    onEdit(draft.trim())
                }
            } else {
                ClipmoButton(
                    label = "Copy",
                    style = ClipmoButtonStyle.PRIMARY,
                    modifier = Modifier.weight(1f),
                    icon = ClipmoIconKind.COPY,
                ) {
                    onCopy(clip)
                }
                if (editable) {
                    ClipmoButton(
                        label = "Edit",
                        style = ClipmoButtonStyle.SECONDARY,
                        modifier = Modifier.weight(1f),
                        icon = ClipmoIconKind.EDIT,
                    ) {
                        editing = true
                    }
                }
                ClipmoButton(
                    label = "Delete",
                    style = ClipmoButtonStyle.TONAL_DANGER,
                    modifier = Modifier.weight(1f),
                    icon = ClipmoIconKind.DELETE,
                ) {
                    onDelete(clip)
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
    val space = ClipmoTheme.spacing

    Box(
        Modifier
            .fillMaxSize()
            .background(colors.scrim)
            .combinedClickable(
                interactionSource = remember { MutableInteractionSource() },
                indication = null,
                onClick = onDismiss,
            ),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            Modifier
                .widthIn(max = 320.dp)
                .padding(space.xl)
                .clip(RoundedCornerShape(ClipmoTheme.shapes.panel))
                .background(colors.surfaceContainerLow)
                .border(1.dp, colors.border, RoundedCornerShape(ClipmoTheme.shapes.panel))
                .combinedClickable(
                    interactionSource = remember { MutableInteractionSource() },
                    indication = null,
                    onClick = {},
                )
                .padding(space.lg),
        ) {
            BasicText(title, style = ClipmoTheme.typography.title.copy(color = colors.textPrimary))
            Spacer(Modifier.height(space.sm))
            BasicText(
                message,
                style = ClipmoTheme.typography.bodyMedium.copy(color = colors.textSecondary),
                maxLines = 3,
                overflow = TextOverflow.Ellipsis,
            )
            Spacer(Modifier.height(space.lg))
            Row(horizontalArrangement = Arrangement.spacedBy(space.sm)) {
                ClipmoButton("Cancel", ClipmoButtonStyle.GHOST, Modifier.weight(1f), onClick = onDismiss)
                ClipmoButton(confirmLabel, ClipmoButtonStyle.DANGER, Modifier.weight(1f), icon = ClipmoIconKind.DELETE, onClick = onConfirm)
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

private fun sourceLabel(source: String?): String = when {
    source.isNullOrBlank() || source == "clipboard" -> "This phone"
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

private fun kindLabel(kind: ClipKind): String = when (kind) {
    ClipKind.TEXT -> "Text"
    ClipKind.URL -> "Link"
    ClipKind.IMAGE -> "Image"
    ClipKind.FILE -> "File"
}

private fun iconFor(kind: ClipKind): ClipmoIconKind = when (kind) {
    ClipKind.TEXT -> ClipmoIconKind.TEXT
    ClipKind.URL -> ClipmoIconKind.LINK
    ClipKind.IMAGE -> ClipmoIconKind.IMAGE
    ClipKind.FILE -> ClipmoIconKind.FILE
}

private fun ClipRecord.isLocalTo(localDeviceId: String): Boolean =
    if (!originDevice.isNullOrBlank()) originDevice == localDeviceId
    else source.isNullOrBlank() || source == "clipboard"

private const val CLIP_PREVIEW_MAX_CHARS = 120

private fun clipPreview(clip: ClipRecord): String = when (clip.kind) {
    ClipKind.IMAGE -> "Image"
    ClipKind.FILE -> clip.content.substringAfterLast('/')
    else -> clip.content.trim().let {
        if (it.length > CLIP_PREVIEW_MAX_CHARS) it.take(CLIP_PREVIEW_MAX_CHARS).trimEnd() + "…" else it
    }
}

