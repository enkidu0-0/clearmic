package dev.clearmic.ui.components

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.ui.draw.alpha
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.unit.dp
import dev.clearmic.ui.theme.Ink
import dev.clearmic.ui.theme.Radius
import dev.clearmic.ui.theme.Space

private val pill = RoundedCornerShape(Radius.Pill)
private val keyBrush = Brush.verticalGradient(listOf(Color.White, Ink.Paper))

/** White card stock with a hairline and a soft lift. */
fun Modifier.panel(): Modifier = this
    .shadow(8.dp, RoundedCornerShape(Radius.Card), ambientColor = Ink.Ink.copy(alpha = 0.10f), spotColor = Ink.Ink.copy(alpha = 0.14f))
    .background(Ink.Card, RoundedCornerShape(Radius.Card))
    .border(1.dp, Ink.Rule, RoundedCornerShape(Radius.Card))

/** A recessed bed for the lanes and for choice groups. */
fun Modifier.bed(shape: androidx.compose.ui.graphics.Shape = RoundedCornerShape(Radius.Part)): Modifier = this
    .background(Ink.Paper2, shape)
    .border(1.dp, Ink.Rule, shape)

/** The one filled control on a screen. */
@Composable
fun PrimaryPill(text: String, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val source = remember { MutableInteractionSource() }
    Box(
        modifier
            .heightIn(min = 56.dp)
            .shadow(6.dp, pill, ambientColor = Ink.Accent.copy(alpha = 0.3f), spotColor = Ink.Accent.copy(alpha = 0.4f))
            .background(Ink.Accent, pill)
            .pressable(source, Role.Button, onClick)
            .padding(horizontal = Space.Xl),
        contentAlignment = Alignment.Center,
    ) { Text(text, style = MaterialTheme.typography.labelLarge, color = Ink.AccentInk) }
}

/** Outlined, quieter than the primary. */
@Composable
fun OutlinePill(text: String, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val source = remember { MutableInteractionSource() }
    Box(
        modifier
            .heightIn(min = 48.dp)
            .shadow(3.dp, pill, ambientColor = Ink.Ink.copy(alpha = 0.08f), spotColor = Ink.Ink.copy(alpha = 0.12f))
            .background(keyBrush, pill)
            .border(BorderStroke(1.dp, Ink.Rule), pill)
            .pressable(source, Role.Button, onClick)
            .padding(horizontal = Space.Lg),
        contentAlignment = Alignment.Center,
    ) { Text(text, style = MaterialTheme.typography.labelLarge, color = Ink.Ink) }
}

/** A choice in a group: the raised fill and the teal edge fade in when it is chosen. */
@Composable
fun ChoicePill(text: String, selected: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val source = remember { MutableInteractionSource() }
    val fill by animateFloatAsState(if (selected) 1f else 0f, tween(300, easing = EaseOutExpo), label = "choiceFill")
    val edge by animateColorAsState(if (selected) Ink.Accent else Ink.Rule, tween(300), label = "choiceEdge")
    val label by animateColorAsState(if (selected) Ink.AccentDeep else Ink.Muted, tween(300), label = "choiceText")
    Box(
        modifier
            .heightIn(min = 48.dp)
            .semantics { role = Role.RadioButton; this.selected = selected }
            .pressable(source, Role.RadioButton, onClick),
        contentAlignment = Alignment.Center,
    ) {
        Box(Modifier.matchParentSize().alpha(fill).background(keyBrush, pill))
        Box(Modifier.matchParentSize().border(BorderStroke(1.dp, edge), pill))
        Text(text, style = MaterialTheme.typography.labelLarge, color = label, modifier = Modifier.padding(horizontal = Space.Lg))
    }
}

/** Status text in a hairline pill. */
@Composable
fun StatusPill(text: String, error: Boolean = false) {
    val shape = pill
    Box(
        Modifier
            .then(if (error) Modifier.border(BorderStroke(1.dp, Ink.Error), shape) else Modifier.background(Ink.AccentSoft, shape))
            .padding(horizontal = Space.Md, vertical = 6.dp),
    ) { Text(text, style = MaterialTheme.typography.labelMedium, color = if (error) Ink.Error else Ink.AccentDeep) }
}

@Composable
fun SwitchRow(label: String, help: String?, checked: Boolean, onChange: (Boolean) -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .clickable(role = Role.Switch) { onChange(!checked) }
            .padding(vertical = Space.Xs),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        androidx.compose.foundation.layout.Column(Modifier.weight(1f)) {
            Text(label, style = MaterialTheme.typography.bodyLarge)
            if (help != null) Text(help, style = MaterialTheme.typography.bodyMedium, color = Ink.Muted)
        }
        Switch(
            checked = checked, onCheckedChange = null,
            colors = SwitchDefaults.colors(
                checkedThumbColor = Ink.AccentInk, checkedTrackColor = Ink.Accent, checkedBorderColor = Ink.Accent,
                uncheckedThumbColor = Ink.MetalHi, uncheckedTrackColor = Ink.Paper3, uncheckedBorderColor = Ink.Rule,
            ),
        )
    }
}

@Composable
fun LabeledSlider(label: String, valueText: String, value: Float, range: ClosedFloatingPointRange<Float>, onChange: (Float) -> Unit) {
    androidx.compose.foundation.layout.Column {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text(label, style = MaterialTheme.typography.labelMedium, color = Ink.Muted)
            Text(valueText, style = MaterialTheme.typography.labelMedium)
        }
        Slider(
            value = value, onValueChange = onChange, valueRange = range,
            colors = SliderDefaults.colors(
                thumbColor = Ink.MetalHi, activeTrackColor = Ink.Accent, inactiveTrackColor = Ink.Paper3,
            ),
        )
    }
}
