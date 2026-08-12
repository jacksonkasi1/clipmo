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
    val xs: Dp = 6.dp,
    val sm: Dp = 8.dp,
    val md: Dp = 12.dp,
    val lg: Dp = 16.dp,
    val xl: Dp = 20.dp,
    val xxl: Dp = 28.dp,
)

@Immutable
data class ClipmoShapes(
    val pill: Dp = 50.dp,
    val search: Dp = 8.dp,
    val card: Dp = 10.dp,
    val panel: Dp = 12.dp,
)

@Immutable
data class ClipmoDimensions(
    val topBarHeight: Dp = 48.dp,
    val searchHeight: Dp = 40.dp,
    val chipHeight: Dp = 28.dp,
    val clipMinHeight: Dp = 62.dp,
    val thumbnail: Dp = 38.dp,
    val icon: Dp = 18.dp,
    val touch: Dp = 42.dp,
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

private val darkColors = ClipmoColors(
    background = Color(0xFF050505),
    surface = Color(0xFF171719),
    surfaceRaised = Color(0xFF242427),
    surfacePressed = Color(0xFF303034),
    textPrimary = Color(0xFFF5F5F2),
    textSecondary = Color(0xFFC5C5C0),
    textMuted = Color(0xFF88888B),
    border = Color(0xFF2B2B2E),
    accent = Color(0xFF78F13D),
    accentMuted = Color(0xFF244515),
    danger = Color(0xFFFF6B68),
    scrim = Color(0xCC000000),
)

private val lightColors = ClipmoColors(
    background = Color(0xFFF5F6F2),
    surface = Color(0xFFFFFFFF),
    surfaceRaised = Color(0xFFE9EAE5),
    surfacePressed = Color(0xFFDEDFDA),
    textPrimary = Color(0xFF11120F),
    textSecondary = Color(0xFF454740),
    textMuted = Color(0xFF73766E),
    border = Color(0xFFD9DBD4),
    accent = Color(0xFF49C514),
    accentMuted = Color(0xFFDDF7D0),
    danger = Color(0xFFC92F2B),
    scrim = Color(0x66000000),
)

private val typography = ClipmoTypography(
    brand = TextStyle(fontFamily = FontFamily.Serif, fontSize = 19.sp, fontWeight = FontWeight.Medium),
    title = TextStyle(fontFamily = FontFamily.SansSerif, fontSize = 20.sp, fontWeight = FontWeight.SemiBold),
    section = TextStyle(fontFamily = FontFamily.SansSerif, fontSize = 13.sp, fontWeight = FontWeight.Medium),
    body = TextStyle(fontFamily = FontFamily.SansSerif, fontSize = 14.sp, fontWeight = FontWeight.Normal),
    label = TextStyle(fontFamily = FontFamily.SansSerif, fontSize = 12.sp, fontWeight = FontWeight.Medium),
    metadata = TextStyle(fontFamily = FontFamily.SansSerif, fontSize = 10.sp, fontWeight = FontWeight.Normal),
)
