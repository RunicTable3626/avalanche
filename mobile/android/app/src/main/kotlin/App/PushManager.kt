package net.theavalanche.app

import android.content.Context
import com.google.android.gms.common.ConnectionResult
import com.google.android.gms.common.GoogleApiAvailabilityLight
import com.google.firebase.messaging.FirebaseMessaging
import org.unifiedpush.android.connector.UnifiedPush

// ---------------------------------------------------------------------------
// PushManager
//
// Android analog of iOS Sources/App/PushManager.swift.
// iOS uses APNs (UNUserNotificationCenter + UIApplication.registerForRemoteNotifications).
// Android picks a push transport at registration time (docs/15):
//
//   • Google Play Services present  → FCM (the standard path)
//   • else a UnifiedPush distributor installed → UnifiedPush (degoogled phones)
//   • else                          → no push; the app relies on its foreground
//                                     WebSocket while open (keepalive fallback is
//                                     a deferred follow-up).
//
// Both transports are relay-routed under the same rotating pseudonym: the FCM
// token / UnifiedPush endpoint URL is just the "device token" passed to
// register_push_token, so the homeserver never sees it.
//
// On Android 13+ (API 33+) POST_NOTIFICATIONS is a runtime permission that must
// be requested before banners are shown. Neither transport's registration needs
// a permission — the wakeups are content-free.
//
// The caller (AppViewModel) should call:
//   PushManager.requestPermissionAndRegister(context, appViewModel)
// after a successful login or account creation, mirroring the iOS call site.
// ---------------------------------------------------------------------------

object PushManager {

    /**
     * Pick the best available transport and register for push, then (separately,
     * via MainActivity) request the POST_NOTIFICATIONS permission on Android 13+.
     *
     * Mirrors iOS PushManager.requestPermissionAndRegister(appState:).
     */
    fun requestPermissionAndRegister(context: Context, appViewModel: AppViewModel) {
        registerBestTransport(context, appViewModel)
    }

    /**
     * Called when the Android 13+ POST_NOTIFICATIONS permission result arrives.
     * The permission only gates whether banners are shown, not registration, so
     * either way we (idempotently) ensure a transport is registered.
     */
    fun onPermissionResult(granted: Boolean, context: Context, appViewModel: AppViewModel) {
        if (granted) {
            AppLog.info("PushManager", "POST_NOTIFICATIONS granted")
        } else {
            AppLog.info("PushManager", "POST_NOTIFICATIONS denied — silent pushes still work")
        }
        registerBestTransport(context, appViewModel)
    }

    // -----------------------------------------------------------------------
    // Transport selection
    // -----------------------------------------------------------------------

    private fun registerBestTransport(context: Context, appViewModel: AppViewModel) {
        when {
            playServicesAvailable(context) -> {
                AppLog.info("PushManager", "using FCM transport")
                fetchFcmTokenAndRegister(appViewModel)
            }
            ensureUnifiedPushDistributor(context) -> {
                AppLog.info("PushManager", "using UnifiedPush transport")
                // Re-registering each foreground is recommended: it confirms the
                // distributor is still installed and re-issues the endpoint via
                // UnifiedPushService.onNewEndpoint.
                UnifiedPush.register(context)
            }
            else -> {
                AppLog.warn(
                    "PushManager",
                    "no push transport available (no Play Services, no UnifiedPush " +
                        "distributor) — relying on foreground connection only",
                )
            }
        }
    }

    /** True if Google Play Services (or a compatible provider like microG) is usable. */
    private fun playServicesAvailable(context: Context): Boolean =
        GoogleApiAvailabilityLight.getInstance()
            .isGooglePlayServicesAvailable(context) == ConnectionResult.SUCCESS

    /**
     * Ensure a UnifiedPush distributor is selected. If one was already saved,
     * keep it. Otherwise, if exactly one is installed, auto-pick it; if several
     * are installed we can't choose for the user here, so we skip (a distributor
     * picker is a future UI improvement). Returns true if a distributor is ready.
     */
    private fun ensureUnifiedPushDistributor(context: Context): Boolean {
        val saved = UnifiedPush.getSavedDistributor(context)
        if (!saved.isNullOrEmpty()) return true

        val available = UnifiedPush.getDistributors(context)
        return when {
            available.isEmpty() -> {
                AppLog.info("PushManager", "no UnifiedPush distributor installed")
                false
            }
            available.size == 1 -> {
                UnifiedPush.saveDistributor(context, available.first())
                true
            }
            else -> {
                // Multiple distributors and no saved choice: don't guess.
                AppLog.info(
                    "PushManager",
                    "multiple UnifiedPush distributors installed; none selected " +
                        "(distributor picker not yet implemented)",
                )
                false
            }
        }
    }

    // -----------------------------------------------------------------------
    // FCM path
    // -----------------------------------------------------------------------

    /**
     * Called when FCM issues a new registration token (rotation, or initial
     * fetch). Registers it with all active cores as the "fcm" platform. Invoke
     * from [ActnetFirebaseMessagingService.onNewToken].
     *
     * Mirrors iOS PushManager.didReceiveToken(_:appState:).
     */
    fun didReceiveToken(token: String, appViewModel: AppViewModel) {
        // FCM tokens are environment-agnostic (unlike APNs sandbox vs production).
        registerWithCores(token, platform = "fcm", appViewModel = appViewModel)
    }

    private fun fetchFcmTokenAndRegister(appViewModel: AppViewModel) {
        // Ask FCM for this install's registration token; token fetch is async and
        // needs no notification permission (wakeups are data-only). onNewToken
        // handles later rotations.
        FirebaseMessaging.getInstance().token
            .addOnCompleteListener { task ->
                if (!task.isSuccessful) {
                    AppLog.warn("PushManager", "FCM getToken failed: ${task.exception?.message}")
                    return@addOnCompleteListener
                }
                val token = task.result
                if (token.isNullOrEmpty()) {
                    AppLog.warn("PushManager", "FCM returned an empty token")
                    return@addOnCompleteListener
                }
                didReceiveToken(token, appViewModel)
            }
    }

    // -----------------------------------------------------------------------
    // UnifiedPush path
    // -----------------------------------------------------------------------

    /**
     * Called by [UnifiedPushService.onNewEndpoint] when the distributor issues
     * (or rotates) our endpoint URL. The URL is registered with all active cores
     * as the "unifiedpush" platform; the relay POSTs wakeups to it.
     */
    fun onUnifiedPushEndpoint(endpoint: String, appViewModel: AppViewModel) {
        registerWithCores(endpoint, platform = "unifiedpush", appViewModel = appViewModel)
    }

    // -----------------------------------------------------------------------
    // Shared
    // -----------------------------------------------------------------------

    private fun registerWithCores(token: String, platform: String, appViewModel: AppViewModel) {
        // Log only a short prefix — the token / endpoint URL is a capability
        // (anyone holding it can wake this device) and must not leak into logcat.
        val preview = if (token.length > 8) "${token.take(8)}…" else token
        AppLog.info("PushManager", "$platform registration: $preview")

        // RELAY_URL is baked into BuildConfig (see app/build.gradle.kts), mirroring
        // how iOS reads it from Info.plist.
        val relayUrl = BuildConfig.RELAY_URL
        if (relayUrl.isEmpty()) {
            AppLog.warn(
                "PushManager",
                "RELAY_URL is empty — push registration skipped. Set RELAY_URL and rebuild.",
            )
            return
        }

        // Neither FCM nor UnifiedPush has APNs's sandbox/production split; pass
        // "production" so the relay's environment routing is a no-op for them.
        appViewModel.registerPushTokenWithCores(
            token = token,
            platform = platform,
            relayUrl = relayUrl,
            environment = "production",
        )
    }
}
