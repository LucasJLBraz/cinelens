package io.github.lucasjlbraz.cinelens

import android.app.Activity
import android.content.Intent
import android.webkit.WebView
import androidx.activity.OnBackPressedCallback
import androidx.appcompat.app.AppCompatActivity
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

/** Keeps the obra of the tapped notification, as the notification plugin tells only a page already listening. */
@TauriPlugin
class TocadaPlugin(private val activity: Activity) : Plugin(activity) {
  private var webView: WebView? = null
  private var obra: Long? = null

  // A page opened with no touch since the last one: the WebView's own back would skip Início and
  // leave the app, as it does with history no one touched. The page's own back does not.
  private val voltar = object : OnBackPressedCallback(false) {
    override fun handleOnBackPressed() {
      isEnabled = false
      // Already back on a tab, by the page's own button: back does what it always does there.
      webView?.evaluateJavascript("history.state ? (history.back(), true) : false") { voltou ->
        if (voltou != "true") (activity as AppCompatActivity).onBackPressedDispatcher.onBackPressed()
      }
    }
  }

  override fun load(webView: WebView) {
    this.webView = webView
    (activity as AppCompatActivity).onBackPressedDispatcher.addCallback(activity as AppCompatActivity, voltar)
    guardar(activity.intent)
  }

  override fun onNewIntent(intent: Intent) {
    guardar(intent)
    if (obra != null) webView?.evaluateJavascript("window.notificacaoTocada?.()", null)
  }

  private fun guardar(intent: Intent?) {
    // Recents start the activity with the intent of last time: that tap was already answered.
    if (intent == null || intent.flags and Intent.FLAG_ACTIVITY_LAUNCHED_FROM_HISTORY != 0) return
    // The notification plugin's key; `notify` makes the obra the notification's id, whose
    // high bit says série.
    if (intent.hasExtra("NotificationId")) obra = intent.getIntExtra("NotificationId", 0).toLong() and 0xFFFFFFFFL
  }

  @Command
  fun tocada(invoke: Invoke) {
    val resposta = JSObject()
    resposta.put("obra", obra)
    if (obra != null) voltar.isEnabled = true
    obra = null
    // Now and not on loading: the notification plugin reads it to dismiss the notification.
    activity.intent?.removeExtra("NotificationId")
    invoke.resolve(resposta)
  }
}
