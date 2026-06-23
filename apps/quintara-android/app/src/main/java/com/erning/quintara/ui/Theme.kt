package com.erning.quintara.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

private val LightColors = lightColorScheme(
    primary = Color(0xFF747CF4),
    onPrimary = Color(0xFF101225),
    secondary = Color(0xFFE2B96F),
    surface = Color(0xFFF6F7FA),
    onSurface = Color(0xFF20222E),
    surfaceVariant = Color(0xFFFFFFFF),
    outline = Color(0xFFE4E6EE),
)

@Composable
fun QuintaraTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = LightColors,
        typography = MaterialTheme.typography,
        content = content,
    )
}
