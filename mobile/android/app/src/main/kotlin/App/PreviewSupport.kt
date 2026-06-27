package net.theavalanche.app

import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext

// ---------------------------------------------------------------------------
// Preview support
//
// Jetpack Compose @Preview functions run without an Activity or a real
// ViewModelStoreOwner, so they cannot use `viewModel()` to obtain an
// AppViewModel. AppViewModel only needs an application Context and performs no
// network/FFI work at construction time, so we can build one directly from
// LocalContext and seed it with sample data for a realistic, deterministic
// preview. This is the single helper that all @Preview composables use.
// ---------------------------------------------------------------------------

/**
 * Build an [AppViewModel] suitable for use inside an `@Preview`. The returned
 * instance does no I/O until one of its action methods is called (which previews
 * never do), and is pre-seeded with the supplied [accounts], [conversations], and
 * per-conversation [messagesByConversation].
 */
@Composable
fun rememberPreviewAppViewModel(
    accounts: List<Account> = emptyList(),
    conversations: List<Conversation> = emptyList(),
    messagesByConversation: Map<String, List<Message>> = emptyMap(),
): AppViewModel {
    val context = LocalContext.current.applicationContext
    return remember {
        AppViewModel(context).apply {
            seedForPreview(
                accounts = accounts,
                conversations = conversations,
                messagesByConversation = messagesByConversation,
            )
        }
    }
}
