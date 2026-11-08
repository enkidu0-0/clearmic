package dev.clearmic.ui.screens

import android.Manifest
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import dev.clearmic.engine.PairedPc
import dev.clearmic.engine.Protocol
import dev.clearmic.ui.components.OutlinePill
import dev.clearmic.ui.components.PrimaryPill
import dev.clearmic.ui.components.QrScanner
import dev.clearmic.ui.components.Reveal
import dev.clearmic.ui.theme.Ink
import dev.clearmic.ui.theme.Radius
import dev.clearmic.ui.theme.Space

/** Scan the code on the PC screen (clearmic://host:port?key=...). Typing it is the fallback. */
@Composable
fun PairScreen(onPaired: (PairedPc) -> Unit) {
    val context = LocalContext.current
    var hasCamera by remember {
        mutableStateOf(ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED)
    }
    val askCamera = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { hasCamera = it }
    var scanError by remember { mutableStateOf<String?>(null) }
    var done by remember { mutableStateOf(false) }

    var host by remember { mutableStateOf("") }
    var port by remember { mutableStateOf("47100") }
    var key by remember { mutableStateOf("") }
    val valid = host.isNotBlank() && port.toIntOrNull() != null && Protocol.hexToKey(key) != null

    Column(
        Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(Space.Md),
        verticalArrangement = Arrangement.spacedBy(Space.Md),
    ) {
        Reveal(0) {
            Row(Modifier.fillMaxWidth().height(56.dp), verticalAlignment = Alignment.CenterVertically) {
                Text("Pair with your PC", style = MaterialTheme.typography.titleMedium)
            }
        }

        Reveal(1) {
        Box(
            Modifier.fillMaxWidth().aspectRatio(1f).clip(RoundedCornerShape(Radius.Card))
                .background(Ink.Paper2).border(1.dp, Ink.Rule, RoundedCornerShape(Radius.Card)),
            contentAlignment = Alignment.Center,
        ) {
            if (hasCamera) {
                QrScanner(
                    onCode = { text ->
                        if (!done) {
                            val pc = PairedPc.parse(text)
                            if (pc != null) { done = true; onPaired(pc) } else scanError = "That is not a ClearMic code"
                        }
                    },
                    modifier = Modifier.fillMaxSize(),
                )
            } else {
                Column(Modifier.padding(Space.Lg), verticalArrangement = Arrangement.spacedBy(Space.Sm), horizontalAlignment = Alignment.CenterHorizontally) {
                    Text("Allow the camera to scan the code on your PC screen", color = Ink.Muted)
                    PrimaryPill("Allow camera", { askCamera.launch(Manifest.permission.CAMERA) })
                }
            }
        }
        }
        if (scanError != null) Text(scanError!!, color = Ink.Error, style = MaterialTheme.typography.bodyMedium)

        Reveal(2) {
        Column(verticalArrangement = Arrangement.spacedBy(Space.Md)) {
        Text("Or type what your PC shows", style = MaterialTheme.typography.labelMedium, color = Ink.Muted)
        Row(horizontalArrangement = Arrangement.spacedBy(Space.Xs)) {
            Field(host, { host = it.trim() }, "Address", Modifier.weight(2f), KeyboardType.Uri)
            Field(port, { port = it.filter(Char::isDigit) }, "Port", Modifier.weight(1f), KeyboardType.Number)
        }
        Field(key, { key = it.trim() }, "Key, 64 characters", Modifier.fillMaxWidth(), KeyboardType.Ascii)
        }
        }

        PrimaryPill(
            "Connect",
            { if (valid) onPaired(PairedPc("my pc", host, port.toInt(), key.lowercase())) },
            Modifier.fillMaxWidth(),
        )
    }
}

@Composable
private fun Field(value: String, onChange: (String) -> Unit, label: String, modifier: Modifier, type: KeyboardType) {
    TextField(
        value = value, onValueChange = onChange, singleLine = true, modifier = modifier,
        label = { Text(label) },
        keyboardOptions = KeyboardOptions(keyboardType = type),
        shape = RoundedCornerShape(Radius.Pill),
        colors = TextFieldDefaults.colors(
            focusedContainerColor = Ink.Card, unfocusedContainerColor = Ink.Card,
            focusedIndicatorColor = Ink.Focus, unfocusedIndicatorColor = Ink.Rule,
            focusedTextColor = Ink.Ink, unfocusedTextColor = Ink.Ink,
            focusedLabelColor = Ink.Muted, unfocusedLabelColor = Ink.Muted, cursorColor = Ink.Accent,
        ),
    )
}
