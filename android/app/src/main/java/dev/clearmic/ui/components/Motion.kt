package dev.clearmic.ui.components

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.slideInVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.animation.animateColorAsState
import dev.clearmic.ui.theme.Ink
import dev.clearmic.ui.theme.Radius
import dev.clearmic.ui.theme.Space
import kotlinx.coroutines.delay

/** The one easing the whole app uses: quick start, long soft landing (same curve as the PC app). */
val EaseOutExpo = CubicBezierEasing(0.16f, 1f, 0.3f, 1f)

/** Content rises and fades in, [index] steps after the screen appears. */
@Composable
fun Reveal(index: Int, content: @Composable () -> Unit) {
    var shown by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { delay(index * 80L); shown = true }
    AnimatedVisibility(
        visible = shown,
        enter = fadeIn(tween(450, easing = EaseOutExpo)) + slideInVertically(tween(600, easing = EaseOutExpo)) { it / 6 },
    ) { content() }
}

/** Shrinks a little while pressed and springs back. */
@Composable
fun Modifier.pressable(source: MutableInteractionSource, role: Role, onClick: () -> Unit): Modifier {
    val pressed by source.collectIsPressedAsState()
    val scale by animateFloatAsState(if (pressed) 0.95f else 1f, spring(dampingRatio = 0.55f, stiffness = Spring.StiffnessMedium), label = "press")
    return this
        .graphicsLayer { scaleX = scale; scaleY = scale }
        .clickable(interactionSource = source, indication = null, role = role, onClick = onClick)
}

/**
 * Bottom navigation: a white bar with one raised pill that glides under the chosen label.
 * [items] are the labels, [selected] the index of the current one.
 */
@Composable
fun NavBar(items: List<String>, selected: Int, onSelect: (Int) -> Unit, modifier: Modifier = Modifier) {
    BoxWithConstraints(
        modifier
            .fillMaxWidth()
            .padding(horizontal = Space.Md, vertical = Space.Sm)
            .panel(),
    ) {
        val pad = 6.dp
        val itemW = (maxWidth - pad * 2) / items.size
        val x by animateDpAsState(itemW * selected, spring(dampingRatio = 0.75f, stiffness = Spring.StiffnessMediumLow), label = "navPill")
        Box(
            Modifier
                .padding(pad)
                .offset(x = x)
                .width(itemW)
                .height(48.dp)
                .shadow(3.dp, androidx.compose.foundation.shape.RoundedCornerShape(Radius.Pill), ambientColor = Ink.Accent.copy(alpha = 0.2f), spotColor = Ink.Accent.copy(alpha = 0.3f))
                .background(Ink.AccentSoft, androidx.compose.foundation.shape.RoundedCornerShape(Radius.Pill)),
        )
        Row(Modifier.padding(pad)) {
            items.forEachIndexed { i, label ->
                val on = i == selected
                val color by animateColorAsState(if (on) Ink.AccentDeep else Ink.Muted, tween(250), label = "navText")
                val source = remember { MutableInteractionSource() }
                Box(
                    Modifier
                        .width(itemW)
                        .height(48.dp)
                        .semantics { role = Role.Tab; this.selected = on }
                        .pressable(source, Role.Tab) { onSelect(i) },
                    contentAlignment = Alignment.Center,
                ) { Text(label, style = MaterialTheme.typography.labelLarge, color = color) }
            }
        }
    }
}
