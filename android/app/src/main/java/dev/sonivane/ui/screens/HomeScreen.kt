package dev.sonivane.ui.screens

import android.Manifest
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sonivane.engine.Link
import dev.sonivane.engine.MicState
import dev.sonivane.engine.MicViewModel
import dev.sonivane.ui.components.ChoicePill
import dev.sonivane.ui.components.EaseOutExpo
import dev.sonivane.ui.components.LabeledSlider
import dev.sonivane.ui.components.Lane
import dev.sonivane.ui.components.OutlinePill
import dev.sonivane.ui.components.PrimaryPill
import dev.sonivane.ui.components.Reveal
import dev.sonivane.ui.components.StatusPill
import dev.sonivane.ui.components.bed
import dev.sonivane.ui.components.panel
import dev.sonivane.ui.theme.Ink
import dev.sonivane.ui.theme.Radius
import dev.sonivane.ui.theme.Space
import kotlin.math.roundToInt

/** Which of the four looks the link has, so animations run when it changes and not on every level tick. */
private fun Link.kind(): Int = when (this) {
    Link.Idle -> 0
    Link.Connecting -> 1
    is Link.Live -> 2
    is Link.Failed -> 3
}

@Composable
fun HomeScreen(state: MicState, vm: MicViewModel, onPair: () -> Unit) {
    val ask = rememberLauncherForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) { granted ->
        if (granted[Manifest.permission.RECORD_AUDIO] == true) vm.start()
    }
    val start = {
        val perms = buildList {
            add(Manifest.permission.RECORD_AUDIO)
            if (Build.VERSION.SDK_INT >= 33) add(Manifest.permission.POST_NOTIFICATIONS)
        }
        ask.launch(perms.toTypedArray())
    }

    Column(
        Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(Space.Md),
        verticalArrangement = Arrangement.spacedBy(Space.Md),
    ) {
        // top bar: wordmark and a status pill that swaps with a slide
        Reveal(0) {
            Row(Modifier.fillMaxWidth().height(56.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.SpaceBetween) {
                Text("Sonivane", style = MaterialTheme.typography.titleMedium)
                AnimatedContent(
                    targetState = state.link,
                    transitionSpec = {
                        (fadeIn(tween(350, easing = EaseOutExpo)) + slideInVertically(tween(450, easing = EaseOutExpo)) { it / 2 }) togetherWith
                            (fadeOut(tween(150)) + slideOutVertically(tween(250)) { -it / 2 })
                    },
                    label = "status",
                ) { link ->
                    when (link) {
                        Link.Idle -> StatusPill(if (state.paired == null) "No PC paired" else "Ready")
                        Link.Connecting -> StatusPill("Connecting")
                        is Link.Live -> StatusPill("Sending to ${link.pcName}")
                        is Link.Failed -> StatusPill("Connection lost", error = true)
                    }
                }
            }
        }

        // the lane card: its content changes with the link, and it glows softly while live
        Reveal(1) {
            val pulse by rememberInfiniteTransition(label = "live").animateFloat(
                initialValue = 0.15f, targetValue = 0.7f,
                animationSpec = infiniteRepeatable(tween(1600, easing = EaseOutExpo), RepeatMode.Reverse), label = "pulse",
            )
            val live = state.link is Link.Live && !state.muted
            Column(
                Modifier.fillMaxWidth().panel().padding(Space.Lg),
                verticalArrangement = Arrangement.spacedBy(Space.Md),
            ) {
                AnimatedContent(
                    targetState = state.link.kind() to state.muted,
                    transitionSpec = {
                        (fadeIn(tween(400, easing = EaseOutExpo)) + slideInVertically(tween(500, easing = EaseOutExpo)) { it / 4 }) togetherWith
                            (fadeOut(tween(150)) + slideOutVertically(tween(200)) { -it / 6 })
                    },
                    label = "headline",
                ) { (kind, muted) ->
                    Text(
                        when (kind) {
                            2 -> if (muted) "Muted" else "Your voice is live"
                            1 -> "Connecting to your PC"
                            3 -> "The PC stopped answering"
                            else -> if (state.paired == null) "Pair with your PC first" else "Ready when you are"
                        },
                        style = MaterialTheme.typography.headlineMedium,
                    )
                }
                AnimatedContent(
                    targetState = state.link.kind(),
                    transitionSpec = { fadeIn(tween(450, easing = EaseOutExpo)) togetherWith fadeOut(tween(150)) },
                    label = "body",
                ) { kind ->
                    when (kind) {
                        2 -> Column(verticalArrangement = Arrangement.spacedBy(Space.Md)) {
                            Box(
                                Modifier.fillMaxWidth().bed().padding(Space.Sm)
                                    .border(1.dp, Ink.Accent.copy(alpha = if (live) pulse else 0f), RoundedCornerShape(Radius.Part)),
                            ) { Lane(state.levels, active = !state.muted, description = "Live level of your microphone") }
                            Text("Noise is removed on the PC, so what you hear in Discord is cleaner than this.", color = Ink.Muted, style = MaterialTheme.typography.bodyMedium)
                        }
                        1 -> Spacer(Modifier.height(96.dp).fillMaxWidth().bed())
                        3 -> Text((state.link as? Link.Failed)?.reason ?: "", color = Ink.Ink2, style = MaterialTheme.typography.bodyLarge)
                        else -> Text(
                            if (state.paired == null) "Open Sonivane on your PC, then scan the code it shows."
                            else "Keep your phone close to your mouth, about a hand's width away.",
                            color = Ink.Ink2, style = MaterialTheme.typography.bodyLarge,
                        )
                    }
                }
            }
        }

        // the one filled action swaps in place
        Reveal(2) {
            AnimatedContent(
                targetState = state.link.kind() to (state.paired == null),
                transitionSpec = { fadeIn(tween(300, easing = EaseOutExpo)) togetherWith fadeOut(tween(120)) },
                label = "action",
            ) { (kind, unpaired) ->
                when (kind) {
                    0 -> if (unpaired) PrimaryPill("Scan PC code", onPair, Modifier.fillMaxWidth()) else PrimaryPill("Start", start, Modifier.fillMaxWidth())
                    1 -> OutlinePill("Cancel", vm::stop, Modifier.fillMaxWidth())
                    2 -> PrimaryPill("Stop", vm::stop, Modifier.fillMaxWidth())
                    else -> PrimaryPill("Try again", start, Modifier.fillMaxWidth())
                }
            }
        }

        Reveal(3) {
            Column(Modifier.fillMaxWidth().panel().padding(Space.Lg), verticalArrangement = Arrangement.spacedBy(Space.Sm)) {
                Row(horizontalArrangement = Arrangement.spacedBy(Space.Xs)) {
                    ChoicePill("Live", selected = !state.muted, onClick = { vm.setMuted(false) })
                    ChoicePill("Muted", selected = state.muted, onClick = { vm.setMuted(true) })
                }
                LabeledSlider("Boost", "${if (state.gainDb > 0) "+" else ""}${state.gainDb.roundToInt()} dB", state.gainDb, -12f..12f, vm::setGain)
            }
        }
    }
}
