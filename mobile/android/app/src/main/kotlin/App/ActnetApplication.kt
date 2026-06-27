package net.theavalanche.app

import android.app.Application
import net.theavalanche.app.BuildConfig
import uniffi.app_core.initLogging

/**
 * Application subclass for the Avalanche (actnet) app.
 *
 * Mirrors the initialization that iOS performs in AppDelegate.application(_:didFinishLaunchingWithOptions:):
 *  - Calls initLogging() once (Rust global logger — panics if called twice; safe here
 *    because Application.onCreate runs exactly once per process).
 *  - Does NOT restore accounts — that is driven by AppViewModel.restoreAccounts(), which
 *    is called from the Compose entry point (MainActivity/RootView) after the ViewModel
 *    is attached to the lifecycle. This mirrors the iOS `.task { await appState.restoreAccounts() }`
 *    placed on RootView.
 *
 * iOS background-push handling (didReceiveRemoteNotification) and APNs token forwarding
 * map to [ActnetFirebaseMessagingService] here, which reaches the running app via
 * [appViewModel]. The notification channel itself is created on API 26+ by
 * NotificationPresenter.createNotificationChannel(), invoked from MainActivity.onCreate.
 */
class ActnetApplication : Application() {

    /**
     * The live top-level ViewModel, published by MainActivity once created.
     * [ActnetFirebaseMessagingService] reads this to forward FCM token rotations
     * and content-free wakeups into the running app. Null when no Activity is alive
     * (e.g. a push delivered to a cold process) — the message is then fetched on the
     * next app launch.
     */
    @Volatile
    var appViewModel: AppViewModel? = null

    override fun onCreate() {
        super.onCreate()

        // Initialize the Rust logger exactly once, before any FFI call.
        // Mirrors iOS:
        //   #if DEBUG
        //   initLogging(filter: "app_core=debug,net=info,store=info,crypto=info")
        //   #else
        //   initLogging(filter: "info")
        //   #endif
        try {
            if (BuildConfig.DEBUG) {
                initLogging(filter = "app_core=debug,net=info,store=info,crypto=info")
            } else {
                initLogging(filter = "info")
            }
        } catch (e: Exception) {
            // Logging init failed (e.g. called twice in robolectric tests).
            // Swallow silently — the app can still run without structured logging.
            android.util.Log.w("ActnetApplication", "initLogging failed (already initialised?): ${e.message}")
        }

        AppLog.info("ActnetApplication", "Process started, logging initialized")
    }
}
