package net.theavalanche.app

import org.unifiedpush.android.connector.FailedReason
import org.unifiedpush.android.connector.PushService
import org.unifiedpush.android.connector.data.PushEndpoint
import org.unifiedpush.android.connector.data.PushMessage

// ---------------------------------------------------------------------------
// UnifiedPushService
//
// Degoogled-Android counterpart to ActnetFirebaseMessagingService (docs/15).
// The UnifiedPush connector (3.x) delivers distributor events to a bound
// PushService rather than a BroadcastReceiver. Same relay-routed pseudonym model
// as FCM: the endpoint URL is just another "device token" handed to
// register_push_token with platform = "unifiedpush", and a wakeup is
// content-free — we only nudge the running app to drain events.
//
// Reaches the running app through ActnetApplication.appViewModel (a Service has
// `application` in scope); null on a cold process, in which case
// registration/sync happens on the next app launch (when PushManager
// re-registers, the distributor re-issues the endpoint).
// ---------------------------------------------------------------------------

class UnifiedPushService : PushService() {

    private val viewModel: AppViewModel?
        get() = (application as? ActnetApplication)?.appViewModel

    /** The distributor issued (or rotated) our endpoint URL — register it. */
    override fun onNewEndpoint(endpoint: PushEndpoint, instance: String) {
        // The endpoint URL is a capability; log only that one arrived, not the URL.
        AppLog.info("UnifiedPush", "onNewEndpoint")
        val vm = viewModel
        if (vm == null) {
            AppLog.info("UnifiedPush", "onNewEndpoint with no active ViewModel — deferring registration")
            return
        }
        PushManager.onUnifiedPushEndpoint(endpoint.url, vm)
    }

    /** A content-free wakeup arrived — drain new events if the app is alive. */
    override fun onMessage(message: PushMessage, instance: String) {
        AppLog.info("UnifiedPush", "wakeup received")
        val vm = viewModel
        if (vm != null) {
            vm.startMessagePolling()
        } else {
            AppLog.info("UnifiedPush", "wakeup with no active ViewModel — will sync on next launch")
        }
    }

    /** Distributor registration failed (e.g. distributor uninstalled). */
    override fun onRegistrationFailed(reason: FailedReason, instance: String) {
        AppLog.warn("UnifiedPush", "registration failed: $reason — will retry on next foreground")
    }

    /** Distributor dropped our registration. The stale relay pseudonym is reaped
     *  by the relay's GC; re-registration happens on the next foreground. */
    override fun onUnregistered(instance: String) {
        AppLog.info("UnifiedPush", "unregistered by distributor")
    }
}
