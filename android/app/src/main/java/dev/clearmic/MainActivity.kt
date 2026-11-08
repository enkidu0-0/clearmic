package dev.clearmic

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.animation.AnimatedContentTransitionScope
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.NavBackStackEntry
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import dev.clearmic.engine.EngineHolder
import dev.clearmic.engine.MicViewModel
import dev.clearmic.ui.components.EaseOutExpo
import dev.clearmic.ui.components.NavBar
import dev.clearmic.ui.screens.HomeScreen
import dev.clearmic.ui.screens.PairScreen
import dev.clearmic.ui.screens.SettingsScreen
import dev.clearmic.ui.theme.ClearMicTheme
import dev.clearmic.ui.theme.Ink

private val routes = listOf("home", "pair", "settings")
private val labels = listOf("Live", "Phone", "Settings")

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge(
            statusBarStyle = SystemBarStyle.light(android.graphics.Color.TRANSPARENT, android.graphics.Color.TRANSPARENT),
            navigationBarStyle = SystemBarStyle.light(android.graphics.Color.TRANSPARENT, android.graphics.Color.TRANSPARENT),
        )
        EngineHolder.init(this)
        setContent {
            ClearMicTheme {
                val nav = rememberNavController()
                val vm: MicViewModel = viewModel()
                val state by vm.state.collectAsStateWithLifecycle()
                val entry by nav.currentBackStackEntryAsState()
                val current = routes.indexOf(entry?.destination?.route).coerceAtLeast(0)

                fun go(i: Int) {
                    if (i == current) return
                    nav.navigate(routes[i]) {
                        popUpTo(nav.graph.findStartDestination().id) { saveState = true }
                        launchSingleTop = true
                        restoreState = true
                    }
                }

                // screens slide in from the side they sit on in the bar, and fade
                fun AnimatedContentTransitionScope<NavBackStackEntry>.dir(): Int =
                    routes.indexOf(targetState.destination.route).compareTo(routes.indexOf(initialState.destination.route))

                Column(Modifier.fillMaxSize().background(Ink.Paper).safeDrawingPadding()) {
                    Box(Modifier.weight(1f)) {
                        NavHost(
                            nav, startDestination = "home",
                            enterTransition = { slideInHorizontally(tween(450, easing = EaseOutExpo)) { it / 6 * dir() } + fadeIn(tween(350)) },
                            exitTransition = { slideOutHorizontally(tween(300, easing = EaseOutExpo)) { -it / 8 * dir() } + fadeOut(tween(200)) },
                            popEnterTransition = { slideInHorizontally(tween(450, easing = EaseOutExpo)) { it / 6 * dir() } + fadeIn(tween(350)) },
                            popExitTransition = { slideOutHorizontally(tween(300, easing = EaseOutExpo)) { -it / 8 * dir() } + fadeOut(tween(200)) },
                        ) {
                            composable("home") { HomeScreen(state, vm, onPair = { go(1) }) }
                            composable("pair") { PairScreen(onPaired = { vm.pair(it); go(0) }) }
                            composable("settings") { SettingsScreen(state, vm) }
                        }
                    }
                    NavBar(labels, current, onSelect = ::go)
                }
            }
        }
    }
}
