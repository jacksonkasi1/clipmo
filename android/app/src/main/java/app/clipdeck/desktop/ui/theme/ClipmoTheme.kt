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
    val surfaceRaised: Color,
    val surfacePressed: Color,
    val textPrimary: Color,
    val textSecondary: Color,
    val textMuted: Color,
    val border: Color,
    val accent: Color,
    val accentMuted: Color,
    val onAccent: Color,
    val danger: Color,
    val scrim: Color,
)

@Immutable
data class ClipmoTypography(
    val brand: TextStyle,
    val title: TextStyle,
    val section: TextStyle,
    val body: TextStyle,
    val label: TextStyle,
    val metadata: TextStyle,
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
    val pill: Dp = 50.dp,
    val search: Dp = 14.dp,
    val card: Dp = 16.dp,
    val panel: Dp = 20.dp,
    val sheet: Dp = 28.dp,
)

@Immutable
data class ClipmoDimensions(
    val topBarHeight: Dp = 56.dp,
    val searchHeight: Dp = 48.dp,
    val chipHeight: Dp = 40.dp,
    val actionHeight: Dp = 48.dp,
    val clipMinHeight: Dp = 72.dp,
    val thumbnail: Dp = 44.dp,
    val icon: Dp = 20.dp,
    val touch: Dp = 48.dp,
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

// Logo-blue accent (#1A73E8 family) on cool neutral grays; accent stays the
// ~10% brand color while surfaces and text carry the 60/30/10 balance.
private val darkColors = ClipmoColors(
    background = Color(0xFF05070B),
    surface = Color(0xFF161A21),
    surfaceRaised = Color(0xFF222834),
    surfacePressed = Color(0xFF2F3644),
    textPrimary = Color(0xFFF2F4F8),
    textSecondary = Color(0xFFBEC4D0),
    textMuted = Color(0xFF858D9E),
    border = Color(0xFF2A313D),
    accent = Color(0xFF5CA9FF),
    accentMuted = Color(0xFF17263F),
    onAccent = Color(0xFF0A1B33),
    danger = Color(0xFFFF6B68),
    scrim = Color(0xCC000000),
)

private val lightColors = ClipmoColors(
    background = Color(0xFFF4F6FA),
    surface = Color(0xFFFFFFFF),
    surfaceRaised = Color(0xFFEDEFF5),
    surfacePressed = Color(0xFFE1E5EE),
    textPrimary = Color(0xFF10141C),
    textSecondary = Color(0xFF3E4554),
    textMuted = Color(0xFF6F7687),
    border = Color(0xFFDCE0EA),
    accent = Color(0xFF1E7BF0),
    accentMuted = Color(0xFFDEEAFC),
    onAccent = Color(0xFFFFFFFF),
    danger = Color(0xFFD23730),
    scrim = Color(0x66000000),
)

// Four sizes (20/14/12/10), two weights (SemiBold/Normal); hierarchy comes
// from size, weight, and text color opacity — not extra weights.
private val typography = ClipmoTypography(
    brand = TextStyle(fontFamily = FontFamily.Serif, fontSize = 20.sp, fontWeight = FontWeight.SemiBold),
    title = TextStyle(fontFamily = FontFamily.SansSerif, fontSize = 20.sp, fontWeight = FontWeight.SemiBold),
    section = TextStyle(fontFamily = FontFamily.SansSerif, fontSize = 12.sp, fontWeight = FontWeight.SemiBold),
    body = TextStyle(fontFamily = FontFamily.SansSerif, fontSize = 14.sp, fontWeight = FontWeight.Normal),
    label = TextStyle(fontFamily = FontFamily.SansSerif, fontSize = 12.sp, fontWeight = FontWeight.Normal),
    metadata = TextStyle(fontFamily = FontFamily.SansSerif, fontSize = 10.sp, fontWeight = FontWeight.Normal),
)
