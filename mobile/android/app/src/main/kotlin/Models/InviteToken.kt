package net.theavalanche.app

import android.net.Uri
import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.app_core.validateInvite

/// Parsed and server-validated invite token.
/// Mirrors InviteToken.swift — iOS is the reference implementation.
data class InviteToken(
    val token: String,
    val serverUrl: String,
    val serverName: String,
    val inviterDid: String?,
    val postOnboardingRedirect: String?,
    /// Operator's privacy policy URL, resolved by the core as part of invite
    /// validation (no separate server call). null when none is configured or
    /// the value is blank. Onboarding screens show the link only when non-null.
    val privacyPolicyUrl: String?,
) {
    // Mirrors Swift's `var id: String { token }` from Identifiable conformance.
    val id: String get() = token

    companion object {
        /// Parse a go.theavalanche.net invite URL and validate the token with the server.
        /// Mirrors InviteToken.from(url:) — path must be /i/<token> or /invite/<token>.
        /// (Named fromUrl because Kotlin, unlike Swift, has no argument labels to
        /// distinguish this from the raw-token overload.)
        suspend fun fromUrl(url: String): InviteToken {
            val uri = Uri.parse(url)
            val segments = uri.pathSegments.filter { it.isNotEmpty() }
            val action = segments.firstOrNull()
            if (action == null || (action != "i" && action != "invite") || segments.size < 2) {
                throw InviteError.InvalidURL
            }
            val token = segments[1]
            return fromToken(token)
        }

        /// Validate a raw base64url token string with the server.
        /// Calls the UniFFI top-level validateInvite() function on a background thread.
        suspend fun fromToken(token: String): InviteToken {
            val info = withContext(Dispatchers.IO) {
                validateInvite(token = token)
            }
            return InviteToken(
                token = token,
                serverUrl = info.serverUrl,
                serverName = info.serverName,
                inviterDid = info.inviterDid,
                postOnboardingRedirect = info.postOnboardingRedirect,
                privacyPolicyUrl = info.privacyPolicyUrl?.takeIf { it.isNotBlank() },
            )
        }
    }
}

sealed class InviteError(message: String) : Exception(message) {
    /// The URL did not match the expected /i/<token> or /invite/<token> pattern.
    object InvalidURL : InviteError("Invalid invite link")

    /// The server URL embedded in the invite info could not be parsed.
    object InvalidServerUrl : InviteError("Invalid server URL in invite")
}
