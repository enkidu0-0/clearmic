package dev.sonivane.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
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
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sonivane.engine.MicState
import dev.sonivane.engine.MicViewModel
import dev.sonivane.ui.components.ChoicePill
import dev.sonivane.ui.components.Reveal
import dev.sonivane.ui.components.panel
import dev.sonivane.ui.theme.Ink
import dev.sonivane.ui.theme.Radius
import dev.sonivane.ui.theme.Space

@Composable
fun SettingsScreen(state: MicState, vm: MicViewModel) {
    Column(
        Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(Space.Md),
        verticalArrangement = Arrangement.spacedBy(Space.Md),
    ) {
        Reveal(0) {
            Row(Modifier.fillMaxWidth().height(56.dp), verticalAlignment = Alignment.CenterVertically) {
                Text("Settings", style = MaterialTheme.typography.titleMedium)
            }
        }

        Reveal(1) { Section("Sound quality") {
            Row(horizontalArrangement = Arrangement.spacedBy(Space.Xs)) {
                listOf(32, 64, 96).forEach { kbps ->
                    ChoicePill("$kbps kbps", state.bitrateKbps == kbps, { vm.setBitrate(kbps) })
                }
            }
            Text("64 is clear for voice. Use 32 on a weak connection.", color = Ink.Muted, style = MaterialTheme.typography.bodyMedium)
        } }

        Reveal(2) { Section("Keep it running") {
            Text(
                "Sonivane keeps a notification while it is sending so Android does not stop it. On Xiaomi, Oppo and Vivo phones, also allow it to run in the background in battery settings.",
                color = Ink.Ink2, style = MaterialTheme.typography.bodyLarge,
            )
        } }
    }
}

@Composable
private fun Section(title: String, content: @Composable () -> Unit) {
    Column(
        Modifier.fillMaxWidth().panel().padding(Space.Lg),
        verticalArrangement = Arrangement.spacedBy(Space.Sm),
    ) {
        Text(title, style = MaterialTheme.typography.titleMedium)
        content()
    }
}
