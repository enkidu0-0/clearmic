package dev.clearmic.ui.theme

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.Typography
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** Ribbon, dawn colourway: warm white paper, white card stock, one deep teal. Same tokens as desktop/app/ui/tokens.css. */
object Ink {
    val Paper = Color(0xFFFCF9F3)
    val Paper2 = Color(0xFFF5F1E9)
    val Paper3 = Color(0xFFEAE5DB)
    val Card = Color(0xFFFFFEFC)
    val Ink = Color(0xFF13110F)
    val Ink2 = Color(0xFF302D29)
    val Muted = Color(0xFF5E5A53)
    val Rule = Color(0xFFE0DCD4)
    val Rule2 = Color(0xFFECE9E3)
    val Accent = Color(0xFF035F5C)
    val AccentDeep = Color(0xFF004B48)
    val AccentSoft = Color(0xFFD1EBE9)
    val AccentInk = Color(0xFFFAF8F4)
    val Focus = Color(0xFF007974)
    val Error = Color(0xFFBE2323)
    val MetalHi = Color(0xFFF7F5F1)
    val MetalLo = Color(0xFFD1CDC7)
}

object Radius {
    val Card = 20.dp
    val Part = 6.dp
    val Pill = 999.dp
}

object Space {
    val Xs = 8.dp
    val Sm = 12.dp
    val Md = 16.dp
    val Lg = 24.dp
    val Xl = 40.dp
}

// Instrument Sans (display) and Figtree (body) ship as downloadable fonts once the font provider is set up;
// until then the system sans keeps the same weights and sizes.
private val Display = FontFamily.SansSerif
private val Body = FontFamily.SansSerif

private val type = Typography(
    headlineMedium = TextStyle(fontFamily = Display, fontWeight = FontWeight.Normal, fontSize = 30.sp, lineHeight = 32.sp, letterSpacing = (-1).sp),
    titleMedium = TextStyle(fontFamily = Display, fontWeight = FontWeight.Medium, fontSize = 18.sp, lineHeight = 22.sp),
    bodyLarge = TextStyle(fontFamily = Body, fontSize = 16.sp, lineHeight = 24.sp),
    bodyMedium = TextStyle(fontFamily = Body, fontSize = 16.sp, lineHeight = 22.sp),
    labelLarge = TextStyle(fontFamily = Display, fontWeight = FontWeight.Medium, fontSize = 16.sp),
    labelMedium = TextStyle(fontFamily = Display, fontWeight = FontWeight.Medium, fontSize = 14.sp),
    displaySmall = TextStyle(fontFamily = Body, fontWeight = FontWeight.Medium, fontSize = 32.sp, lineHeight = 32.sp),
)

@Composable
fun ClearMicTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = lightColorScheme(
            primary = Ink.Accent, onPrimary = Ink.AccentInk,
            background = Ink.Paper, onBackground = Ink.Ink,
            surface = Ink.Card, onSurface = Ink.Ink,
            surfaceVariant = Ink.Paper2, onSurfaceVariant = Ink.Muted,
            outline = Ink.Rule, outlineVariant = Ink.Rule2, error = Ink.Error,
        ),
        typography = type,
        shapes = Shapes(
            small = RoundedCornerShape(Radius.Pill),
            medium = RoundedCornerShape(Radius.Card),
            large = RoundedCornerShape(Radius.Card),
        ),
        content = content,
    )
}
