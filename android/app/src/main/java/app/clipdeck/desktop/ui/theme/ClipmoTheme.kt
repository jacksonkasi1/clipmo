package app.clipdeck.desktop.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

enum class ClipmoThemeMode { SYSTEM, LIGHT, DARK }

@Immutable
data class ClipmoColors(
    val background: Color,
    val surface: Color,
    val surfaceContainerLowest: Color,
    val surfaceContainerLow: Color,
    val surfaceContainer: Color,
    val surfaceContainerHigh: Color,
    val surfaceContainerHighest: Color,
    val surfaceRaised: Color,
    val surfacePressed: Color,
    val textPrimary: Color,
    val textSecondary: Color,
    val textMuted: Color,
    val border: Color,
    val borderSubtle: Color,
    val accent: Color,
    val accentMuted: Color,
    val onAccent: Color,
    val onAccentMuted: Color,
    val secondary: Color,
    val secondaryContainer: Color,
    val onSecondaryContainer: Color,
    val success: Color,
    val successContainer: Color,
    val onSuccessContainer: Color,
    val warning: Color,
    val warningContainer: Color,
    val onWarningContainer: Color,
    val danger: Color,
    val dangerContainer: Color,
    val onDangerContainer: Color,
    val scrim: Color,
)

@Immutable
data class ClipmoTypography(
    val brand: TextStyle,
    val headline: TextStyle,
    val title: TextStyle,
    val titleSmall: TextStyle,
    val section: TextStyle,
    val body: TextStyle,
    val bodyMedium: TextStyle,
    val bodySmall: TextStyle,
    val label: TextStyle,
    val labelSmall: TextStyle,
    val metadata: TextStyle,
    val code: TextStyle,
    val pairingCode: TextStyle,
)

@Immutable
data class ClipmoSpacing(
    val xxs: Dp = 4.dp,
    val xs: Dp = 8.dp,
    val sm: Dp = 12.dp,
    val md: Dp = 16.dp,
    val lg: Dp = 20.dp,
    val xl: Dp = 24.dp,
    val xxl: Dp = 32.dp,
)

@Immutable
data class ClipmoShapes(
    val xs: Dp = 6.dp,
    val sm: Dp = 10.dp,
    val md: Dp = 14.dp,
    val lg: Dp = 18.dp,
    val xl: Dp = 24.dp,
    val pill: Dp = 50.dp,
    val search: Dp = 16.dp,
    val card: Dp = 14.dp,
    val panel: Dp = 18.dp,
    val sheet: Dp = 28.dp,
)

@Immutable
data class ClipmoDimensions(
    val topBarHeight: Dp = 56.dp,
    val searchHeight: Dp = 48.dp,
    val chipHeight: Dp = 36.dp,
    val deviceTabHeight: Dp = 40.dp,
    val actionHeight: Dp = 46.dp,
    val clipMinHeight: Dp = 64.dp,
    val thumbnail: Dp = 42.dp,
    val icon: Dp = 20.dp,
    val touch: Dp = 44.dp,
    val navBarHeight: Dp = 64.dp,
)

val LocalClipmoColors = staticCompositionLocalOf { darkColors }
val LocalClipmoTypography = staticCompositionLocalOf { typography }
val LocalClipmoSpacing = staticCompositionLocalOf { ClipmoSpacing() }
val LocalClipmoShapes = staticCompositionLocalOf { ClipmoShapes() }
val LocalClipmoDimensions = staticCompositionLocalOf { ClipmoDimensions() }

object ClipmoTheme {
    val colors: ClipmoColors @Composable get() = LocalClipmoColors.current
    val typography: ClipmoTypography @Composable get() = LocalClipmoTypography.current
    val spacing: ClipmoSpacing @Composable get() = LocalClipmoSpacing.current
    val shapes: ClipmoShapes @Composable get() = LocalClipmoShapes.current
    val dimensions: ClipmoDimensions @Composable get() = LocalClipmoDimensions.current
}

@Composable
fun ClipmoTheme(mode: ClipmoThemeMode, content: @Composable () -> Unit) {
    val dark = when (mode) {
        ClipmoThemeMode.SYSTEM -> isSystemInDarkTheme()
        ClipmoThemeMode.LIGHT -> false
        ClipmoThemeMode.DARK -> true
    }
    androidx.compose.runtime.CompositionLocalProvider(
        LocalClipmoColors provides if (dark) darkColors else lightColors,
        LocalClipmoTypography provides typography,
        LocalClipmoSpacing provides ClipmoSpacing(),
        LocalClipmoShapes provides ClipmoShapes(),
        LocalClipmoDimensions provides ClipmoDimensions(),
        content = content,
    )
}

// Material 3 Expressive Dark Tonal Palette:
// Google Material 3 Expressive Dark Palette
private val darkColors = ClipmoColors(
    background = Color(0xFF111318),
    surface = Color(0xFF191C20),
    surfaceContainerLowest = Color(0xFF0C0E12),
    surfaceContainerLow = Color(0xFF1D2026),
    surfaceContainer = Color(0xFF22252B),
    surfaceContainerHigh = Color(0xFF2B2E35),
    surfaceContainerHighest = Color(0xFF353942),
    surfaceRaised = Color(0xFF1F2228),
    surfacePressed = Color(0xFF282B32),
    textPrimary = Color(0xFFE2E2E9),
    textSecondary = Color(0xFFC4C7C5),
    textMuted = Color(0xFF8E918F),
    border = Color(0xFF353940),
    borderSubtle = Color(0xFF26292E),
    accent = Color(0xFFA8C7FA),
    accentMuted = Color(0xFF0842A0),
    onAccent = Color(0xFF041E49),
    onAccentMuted = Color(0xFFD3E3FD),
    secondary = Color(0xFF7FD0FF),
    secondaryContainer = Color(0xFF004C74),
    onSecondaryContainer = Color(0xFFC2E7FF),
    success = Color(0xFF6DD58C),
    successContainer = Color(0xFF0D381E),
    onSuccessContainer = Color(0xFFB8F3C7),
    warning = Color(0xFFFFB951),
    warningContainer = Color(0xFF5B3B00),
    onWarningContainer = Color(0xFFFFDDB3),
    danger = Color(0xFFFFB4AB),
    dangerContainer = Color(0xFF93000A),
    onDangerContainer = Color(0xFFFFDAD6),
    scrim = Color(0x99000000),
)

// Google Material 3 Expressive Light Palette
private val lightColors = ClipmoColors(
    background = Color(0xFFF8F9FA),
    surface = Color(0xFFFFFFFF),
    surfaceContainerLowest = Color(0xFFFFFFFF),
    surfaceContainerLow = Color(0xFFF0F4F9),
    surfaceContainer = Color(0xFFE9EEF6),
    surfaceContainerHigh = Color(0xFFE1E7F0),
    surfaceContainerHighest = Color(0xFFD3DBE5),
    surfaceRaised = Color(0xFFFFFFFF),
    surfacePressed = Color(0xFFE3E8EF),
    textPrimary = Color(0xFF1F1F1F),
    textSecondary = Color(0xFF444746),
    textMuted = Color(0xFF747775),
    border = Color(0xFFE0E3E7),
    borderSubtle = Color(0xFFEEF1F6),
    accent = Color(0xFF0B57D0),
    accentMuted = Color(0xFFD3E3FD),
    onAccent = Color(0xFFFFFFFF),
    onAccentMuted = Color(0xFF041E49),
    secondary = Color(0xFF00639B),
    secondaryContainer = Color(0xFFC2E7FF),
    onSecondaryContainer = Color(0xFF001D35),
    success = Color(0xFF146C2E),
    successContainer = Color(0xFFC4EED0),
    onSuccessContainer = Color(0xFF072710),
    warning = Color(0xFFBA6200),
    warningContainer = Color(0xFFFFE0B8),
    onWarningContainer = Color(0xFF381A00),
    danger = Color(0xFFBA1A1A),
    dangerContainer = Color(0xFFFFDAD6),
    onDangerContainer = Color(0xFF410002),
    scrim = Color(0x66000000),
)

private val typography = ClipmoTypography(
    brand = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontSize = 20.sp,
        fontWeight = FontWeight.Bold,
        letterSpacing = (-0.5).sp,
    ),
    headline = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontSize = 22.sp,
        fontWeight = FontWeight.Bold,
        letterSpacing = (-0.4).sp,
    ),
    title = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontSize = 17.sp,
        fontWeight = FontWeight.SemiBold,
        letterSpacing = (-0.2).sp,
    ),
    titleSmall = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontSize = 15.sp,
        fontWeight = FontWeight.SemiBold,
    ),
    section = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontSize = 12.sp,
        fontWeight = FontWeight.Bold,
        letterSpacing = 0.8.sp,
    ),
    body = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontSize = 14.sp,
        fontWeight = FontWeight.Normal,
        lineHeight = 20.sp,
    ),
    bodyMedium = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontSize = 13.sp,
        fontWeight = FontWeight.Normal,
        lineHeight = 18.sp,
    ),
    bodySmall = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontSize = 12.sp,
        fontWeight = FontWeight.Normal,
    ),
    label = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontSize = 13.sp,
        fontWeight = FontWeight.SemiBold,
    ),
    labelSmall = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontSize = 11.sp,
        fontWeight = FontWeight.SemiBold,
    ),
    metadata = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontSize = 11.sp,
        fontWeight = FontWeight.Normal,
    ),
    code = TextStyle(
        fontFamily = FontFamily.Monospace,
        fontSize = 13.sp,
        fontWeight = FontWeight.Normal,
    ),
    pairingCode = TextStyle(
        fontFamily = FontFamily.Monospace,
        fontSize = 28.sp,
        fontWeight = FontWeight.Bold,
        letterSpacing = 4.sp,
    ),
)

