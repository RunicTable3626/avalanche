import AuthenticationServices
import CryptoKit
import Foundation

/// Manages WebAuthn passkey registration and authentication ceremonies.
///
/// - Registration: Creates a new passkey with the signup server URL as the
///   userHandle, and returns the raw 32-byte PRF output. The Rust core
///   derives both the DID rotation key and the recovery-blob encryption key
///   from this output via HKDF — the iOS side just shuttles the bytes.
/// - Authentication: Retrieves an existing passkey and returns the raw PRF
///   output plus the original signup server URL from the userHandle, which
///   together let the Rust core recompute the DID without any server lookup.
///
/// The relying party is `theavalanche.net` — shared across all actnet servers
/// so passkeys work for recovery regardless of which server the user is on.
@MainActor
final class PasskeyManager: NSObject {

    /// The relying party domain for all actnet passkeys.
    static let relyingParty = "theavalanche.net"

    /// Fixed PRF salt used during both registration and assertion. The
    /// authenticator's PRF output is deterministic for `(passkey, salt)`, so
    /// the same salt always yields the same 32 bytes.
    private static let prfSalt = Data("actnet-recovery-v1".utf8)

    /// Result of a passkey registration ceremony.
    struct RegistrationResult {
        /// Raw 32-byte PRF output. Rust derives rotation key + blob key from this.
        let prfOutput: Data
    }

    /// Result of a passkey authentication ceremony.
    struct AuthenticationResult {
        /// Raw 32-byte PRF output. Rust derives rotation key + blob key from this.
        let prfOutput: Data
        /// The signup server URL stored in the credential's userHandle. Used
        /// to recompute the genesis op and derive the DID.
        let signupServerUrl: String
    }

    private var registrationContinuation: CheckedContinuation<RegistrationResult, Error>?
    private var authenticationContinuation: CheckedContinuation<AuthenticationResult, Error>?

    /// Register a new passkey for a fresh identity.
    ///
    /// - Parameters:
    ///   - signupServerUrl: The homeserver URL the user is signing up at.
    ///     Stored in `user.id` so that recovery can recompute the DID.
    ///   - displayName: Human-readable label shown in the OS passkey picker
    ///     (e.g. `"Sam @ safe-haven.org"`).
    ///   - anchor: The presentation anchor (window) for the system sheet.
    /// - Returns: The raw PRF output.
    func register(
        signupServerUrl: String,
        displayName: String,
        anchor: ASPresentationAnchor
    ) async throws -> RegistrationResult {
        let provider = ASAuthorizationPlatformPublicKeyCredentialProvider(
            relyingPartyIdentifier: Self.relyingParty
        )

        let challenge = Self.generateChallenge()
        let userHandle = Data(signupServerUrl.utf8)

        let request = provider.createCredentialRegistrationRequest(
            challenge: challenge,
            name: displayName,
            userID: userHandle
        )

        // Request PRF eval at creation — the authenticator returns the PRF
        // output alongside the new credential.
        let inputValues = ASAuthorizationPublicKeyCredentialPRFAssertionInput.InputValues(
            saltInput1: Self.prfSalt
        )
        request.prf = .inputValues(inputValues)

        let controller = ASAuthorizationController(authorizationRequests: [request])
        controller.delegate = self
        controller.presentationContextProvider = WindowAnchorProvider(anchor: anchor)

        return try await withCheckedThrowingContinuation { continuation in
            self.registrationContinuation = continuation
            controller.performRequests()
        }
    }

    /// Authenticate with an existing passkey (for recovery).
    ///
    /// The system presents all passkeys stored for `theavalanche.net`.
    /// The user picks one and confirms with biometrics.
    ///
    /// - Parameter anchor: The presentation anchor for the system sheet.
    /// - Returns: The PRF-derived recovery key and the DID from the user handle.
    func authenticate(
        anchor: ASPresentationAnchor
    ) async throws -> AuthenticationResult {
        let provider = ASAuthorizationPlatformPublicKeyCredentialProvider(
            relyingPartyIdentifier: Self.relyingParty
        )

        let challenge = Self.generateChallenge()
        let request = provider.createCredentialAssertionRequest(challenge: challenge)

        // Request PRF extension to re-derive the same symmetric key.
        let inputValues = ASAuthorizationPublicKeyCredentialPRFAssertionInput.InputValues(
            saltInput1: Self.prfSalt
        )
        request.prf = .inputValues(inputValues)

        let controller = ASAuthorizationController(authorizationRequests: [request])
        controller.delegate = self
        controller.presentationContextProvider = WindowAnchorProvider(anchor: anchor)

        return try await withCheckedThrowingContinuation { continuation in
            self.authenticationContinuation = continuation
            controller.performRequests()
        }
    }

    /// Generate a random challenge for the WebAuthn ceremony.
    private static func generateChallenge() -> Data {
        var bytes = [UInt8](repeating: 0, count: 32)
        _ = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        return Data(bytes)
    }
}

// MARK: - ASAuthorizationControllerDelegate

extension PasskeyManager: ASAuthorizationControllerDelegate {

    nonisolated func authorizationController(
        controller: ASAuthorizationController,
        didCompleteWithAuthorization authorization: ASAuthorization
    ) {
        Task { @MainActor in
            if let credential = authorization.credential as? ASAuthorizationPlatformPublicKeyCredentialRegistration {
                handleRegistration(credential)
            } else if let credential = authorization.credential as? ASAuthorizationPlatformPublicKeyCredentialAssertion {
                handleAssertion(credential)
            }
        }
    }

    nonisolated func authorizationController(
        controller: ASAuthorizationController,
        didCompleteWithError error: Error
    ) {
        Task { @MainActor in
            registrationContinuation?.resume(throwing: error)
            registrationContinuation = nil
            authenticationContinuation?.resume(throwing: error)
            authenticationContinuation = nil
        }
    }

    @MainActor
    private func handleRegistration(_ credential: ASAuthorizationPlatformPublicKeyCredentialRegistration) {
        guard let prfOutput = credential.prf, let prfKey = prfOutput.first else {
            registrationContinuation?.resume(
                throwing: PasskeyError.prfNotSupported
            )
            registrationContinuation = nil
            return
        }

        let prfData = prfKey.withUnsafeBytes { Data($0) }
        let result = RegistrationResult(prfOutput: prfData)
        registrationContinuation?.resume(returning: result)
        registrationContinuation = nil
    }

    @MainActor
    private func handleAssertion(_ credential: ASAuthorizationPlatformPublicKeyCredentialAssertion) {
        guard let prfOutput = credential.prf else {
            authenticationContinuation?.resume(
                throwing: PasskeyError.prfNotSupported
            )
            authenticationContinuation = nil
            return
        }

        let prfData = prfOutput.first.withUnsafeBytes { Data($0) }
        let signupServerUrl = String(data: credential.userID, encoding: .utf8) ?? ""
        let result = AuthenticationResult(
            prfOutput: prfData,
            signupServerUrl: signupServerUrl
        )
        authenticationContinuation?.resume(returning: result)
        authenticationContinuation = nil
    }
}

// MARK: - Supporting types

enum PasskeyError: LocalizedError {
    case prfNotSupported
    case cancelled
    case unknown(String)

    var errorDescription: String? {
        switch self {
        case .prfNotSupported:
            return "Your password manager doesn't support the PRF extension needed for recovery. Try a different passkey provider."
        case .cancelled:
            return "Passkey operation was cancelled."
        case .unknown(let msg):
            return msg
        }
    }
}

/// Provides the presentation anchor for the ASAuthorizationController sheet.
private class WindowAnchorProvider: NSObject, ASAuthorizationControllerPresentationContextProviding {
    let anchor: ASPresentationAnchor

    init(anchor: ASPresentationAnchor) {
        self.anchor = anchor
    }

    func presentationAnchor(for controller: ASAuthorizationController) -> ASPresentationAnchor {
        anchor
    }
}
