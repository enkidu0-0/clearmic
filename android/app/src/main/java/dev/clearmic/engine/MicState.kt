package dev.clearmic.engine

/** What the UI shows. */
sealed interface Link {
    data object Idle : Link
    data object Connecting : Link
    data class Live(val pcName: String, val transport: String) : Link
    data class Failed(val reason: String) : Link
}

data class MicState(
    val link: Link = Link.Idle,
    val paired: PairedPc? = null,
    val muted: Boolean = false,
    val gainDb: Float = 0f,
    val bitrateKbps: Int = 64,
    val levels: List<Float> = emptyList(),
)
