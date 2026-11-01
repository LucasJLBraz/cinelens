package io.github.lucasjlbraz.cinelens

import android.app.Activity
import android.content.ClipData
import android.content.Intent
import android.util.Base64
import androidx.core.content.FileProvider
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin
import java.io.File

@InvokeArg
class CompartilharArgs {
  /** The card, a JPEG in base64. */
  var imagem: String = ""
  var texto: String = ""
}

/** Hands a movie's card and text to the system share sheet: WhatsApp, Instagram and the rest. */
@TauriPlugin
class CompartilharPlugin(private val activity: Activity) : Plugin(activity) {
  @Command
  fun compartilhar(invoke: Invoke) {
    try {
      val args = invoke.parseArgs(CompartilharArgs::class.java)
      // Always the same name: only the last card stays in the cache.
      val file = File(activity.cacheDir, "cinelens.jpg")
      file.writeBytes(Base64.decode(args.imagem, Base64.DEFAULT))
      val uri = FileProvider.getUriForFile(activity, "${activity.packageName}.fileprovider", file)
      val send = Intent(Intent.ACTION_SEND).apply {
        type = "image/jpeg"
        putExtra(Intent.EXTRA_STREAM, uri)
        putExtra(Intent.EXTRA_TEXT, args.texto)
        clipData = ClipData.newRawUri(null, uri)
        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
      }
      activity.startActivity(Intent.createChooser(send, null))
      invoke.resolve()
    } catch (e: Exception) {
      invoke.reject(e.message ?: "share failed")
    }
  }
}
