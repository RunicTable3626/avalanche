package net.theavalanche.app

import com.google.firebase.messaging.FirebaseMessagingService
import com.google.firebase.messaging.RemoteMessage

// ---------------------------------------------------------------------------
// ActnetFirebaseMessagingService
//
// Receives FCM callbacks. Android analog of the iOS AppDelegate push hooks
// (didRegisterForRemoteNotificationsWithDeviceToken / didReceiveRemoteNotification).
//
// The relay only ever sends content-free wakeups (docs/15) — there is no message
// body in the payload. The job here is simply to nudge the running app to drain
// new events over its existing WebSocket/poll loop; NotificationPresenter then
// raises any user-visible banners as messages decrypt.
//
// Reaches the running app through ActnetApplication.appViewModel. If that is null
// (push delivered to a cold process with no Activity yet), there is nothing to
// drive synchronously here — the messages are fetched on the next app launch.
// ---------------------------------------------------------------------------

class ActnetFirebaseMessagingService : FirebaseMessagingService() {

    private val appViewModel: AppViewModel?
        get() = (application as? ActnetApplication)?.appViewModel

    /** FCM rotated this install's registration token — re-register with the relay. */
    override fun onNewToken(token: String) {
        AppLog.info("FCM", "onNewToken")
        val vm = appViewModel
        if (vm == null) {
            // No live app to enumerate cores; registration happens on next login
            // via PushManager.requestPermissionAndRegister.
            AppLog.info("FCM", "onNewToken with no active ViewModel — deferring registration")
            return
        }
        PushManager.didReceiveToken(token, vm)
    }

    /** A content-free wakeup arrived — drain new events if the app is alive. */
    override fun onMessageReceived(message: RemoteMessage) {
        AppLog.info("FCM", "wakeup received (from=${message.from})")
        val vm = appViewModel
        if (vm != null) {
            // Idempotent: ensures the per-account connection/event loops are running
            // so pending messages are fetched and decrypted.
            vm.startMessagePolling()
        } else {
            AppLog.info("FCM", "wakeup with no active ViewModel — will sync on next launch")
        }
    }
}
