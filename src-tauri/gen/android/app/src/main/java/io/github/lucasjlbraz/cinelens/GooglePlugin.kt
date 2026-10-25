package io.github.lucasjlbraz.cinelens

import android.app.Activity
import androidx.activity.result.ActivityResult
import androidx.activity.result.IntentSenderRequest
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import com.google.android.gms.auth.api.identity.AuthorizationRequest
import com.google.android.gms.auth.api.identity.AuthorizationResult
import com.google.android.gms.auth.api.identity.Identity
import com.google.android.gms.common.api.Scope

@InvokeArg
class AuthorizeArgs {
  /** Whether the owner may be shown the consent screen; background syncs never do. */
  var interactive: Boolean = false
}

/**
 * Access to the owner's Drive appDataFolder through Play Services: the consent screen is the
 * system's, and Play Services keeps the grant and refreshes the access token.
 */
@TauriPlugin
class GooglePlugin(private val activity: Activity) : Plugin(activity) {
  private val request = AuthorizationRequest.builder()
    .setRequestedScopes(listOf(Scope("https://www.googleapis.com/auth/drive.appdata")))
    .build()

  @Command
  fun authorize(invoke: Invoke) {
    val args = invoke.parseArgs(AuthorizeArgs::class.java)
    Identity.getAuthorizationClient(activity).authorize(request)
      .addOnSuccessListener { result ->
        val pending = result.pendingIntent
        when {
          !result.hasResolution() -> resolve(invoke, result)
          // The Rust side forgets the grant on this exact message.
          !args.interactive || pending == null -> invoke.reject("sign-in needed")
          else -> startIntentSenderForResult(invoke, IntentSenderRequest.Builder(pending.intentSender).build(), "authorized")
        }
      }
      .addOnFailureListener { invoke.reject(it.message ?: "authorization failed") }
  }

  @ActivityCallback
  private fun authorized(invoke: Invoke, result: ActivityResult) {
    try {
      resolve(invoke, Identity.getAuthorizationClient(activity).getAuthorizationResultFromIntent(result.data))
    } catch (e: Exception) {
      invoke.reject(e.message ?: "sign-in cancelled")
    }
  }

  private fun resolve(invoke: Invoke, result: AuthorizationResult) {
    val token = result.accessToken ?: return invoke.reject("no access token")
    invoke.resolve(JSObject().put("accessToken", token))
  }
}
