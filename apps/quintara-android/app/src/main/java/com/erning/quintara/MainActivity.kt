package com.erning.quintara

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import com.erning.quintara.ui.QuintaraApp
import com.erning.quintara.ui.QuintaraTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            QuintaraTheme {
                QuintaraApp()
            }
        }
    }
}
