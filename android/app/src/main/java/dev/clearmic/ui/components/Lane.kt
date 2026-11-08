package dev.clearmic.ui.components

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import dev.clearmic.ui.theme.Ink

/**
 * A lane of level bars, newest on the right. [levels] are peak values 0..1.
 * [active] draws the played part in violet; otherwise in the soft violet.
 */
@Composable
fun Lane(levels: List<Float>, active: Boolean, description: String, modifier: Modifier = Modifier) {
    val color: Color = if (active) Ink.Accent else Ink.Muted
    Canvas(
        modifier
            .fillMaxWidth()
            .height(96.dp)
            .semantics { contentDescription = description },
    ) {
        val bar = 3.dp.toPx()
        val gap = 2.dp.toPx()
        val n = (size.width / (bar + gap)).toInt()
        val shown = levels.takeLast(n)
        shown.forEachIndexed { i, v ->
            val h = (v.coerceIn(0f, 1f) * size.height).coerceAtLeast(2.dp.toPx())
            val x = size.width - (shown.size - i) * (bar + gap)
            drawRoundRect(color.copy(alpha = 0.25f + 0.75f * (i + 1) / shown.size), Offset(x, (size.height - h) / 2f), Size(bar, h), CornerRadius(bar / 2f))
        }
    }
}
