package io.github.lucasjlbraz.cinelens

import android.content.Intent
import android.os.Bundle
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    // Android restored a killed activity with the intent of last time: that tap was already answered.
    if (savedInstanceState != null) intent.removeExtra("NotificationId")
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }

  // A tap that restarts a killed app arrives here before any plugin loads: they read it from `intent`.
  override fun onNewIntent(intent: Intent) {
    setIntent(intent)
    super.onNewIntent(intent)
  }
}
