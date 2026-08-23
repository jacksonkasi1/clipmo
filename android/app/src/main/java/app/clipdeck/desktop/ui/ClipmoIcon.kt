package app.clipdeck.desktop.ui

import androidx.compose.material.Icon
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.outlined.Add
import androidx.compose.material.icons.outlined.Check
import androidx.compose.material.icons.outlined.Computer
import androidx.compose.material.icons.outlined.ContentCopy
import androidx.compose.material.icons.outlined.ContentPaste
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material.icons.outlined.DeleteOutline
import androidx.compose.material.icons.outlined.Devices
import androidx.compose.material.icons.outlined.Edit
import androidx.compose.material.icons.outlined.FolderOpen
import androidx.compose.material.icons.outlined.Image
import androidx.compose.material.icons.outlined.Link
import androidx.compose.material.icons.outlined.Refresh
import androidx.compose.material.icons.outlined.Search
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material.icons.outlined.Smartphone
import androidx.compose.material.icons.outlined.StarOutline
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material.icons.automirrored.outlined.InsertDriveFile
import androidx.compose.material.icons.automirrored.outlined.TextSnippet
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color

enum class ClipmoIconKind {
    CLIPBOARD, SEARCH, SETTINGS, COPY, STAR, STAR_FILLED, TEXT, LINK, IMAGE, FILE, DEVICE,
    DESKTOP, PHONE, COLLECTION, PLUS, BACK, DELETE, CHECK, REFRESH, EDIT, CLOSE,
}

@Composable
fun ClipmoIcon(kind: ClipmoIconKind, color: Color, modifier: Modifier = Modifier) {
    val image = when (kind) {
        ClipmoIconKind.CLIPBOARD -> Icons.Outlined.ContentPaste
        ClipmoIconKind.SEARCH -> Icons.Outlined.Search
        ClipmoIconKind.SETTINGS -> Icons.Outlined.Settings
        ClipmoIconKind.COPY -> Icons.Outlined.ContentCopy
        ClipmoIconKind.STAR -> Icons.Outlined.StarOutline
        ClipmoIconKind.STAR_FILLED -> Icons.Filled.Star
        ClipmoIconKind.TEXT -> Icons.AutoMirrored.Outlined.TextSnippet
        ClipmoIconKind.LINK -> Icons.Outlined.Link
        ClipmoIconKind.IMAGE -> Icons.Outlined.Image
        ClipmoIconKind.FILE -> Icons.AutoMirrored.Outlined.InsertDriveFile
        ClipmoIconKind.DEVICE -> Icons.Outlined.Devices
        ClipmoIconKind.DESKTOP -> Icons.Outlined.Computer
        ClipmoIconKind.PHONE -> Icons.Outlined.Smartphone
        ClipmoIconKind.COLLECTION -> Icons.Outlined.FolderOpen
        ClipmoIconKind.PLUS -> Icons.Outlined.Add
        ClipmoIconKind.BACK -> Icons.AutoMirrored.Outlined.ArrowBack
        ClipmoIconKind.DELETE -> Icons.Outlined.DeleteOutline
        ClipmoIconKind.CHECK -> Icons.Outlined.Check
        ClipmoIconKind.REFRESH -> Icons.Outlined.Refresh
        ClipmoIconKind.EDIT -> Icons.Outlined.Edit
        ClipmoIconKind.CLOSE -> Icons.Outlined.Close
    }
    Icon(imageVector = image, contentDescription = null, tint = color, modifier = modifier)
}

