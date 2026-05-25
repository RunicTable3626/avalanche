import SwiftUI
import AuthenticationServices

struct PasskeyExplainerView: View {
    @EnvironmentObject var appState: AppState
    let inviteToken: InviteToken
    let displayName: String

    @State private var isRegistering = false
    @State private var errorMessage: String?

    var body: some View {
        VStack(spacing: 24) {
            Spacer()

            Text("Create a passkey to protect this identity")
                .font(.title2)
                .fontWeight(.semibold)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)

            // Profile preview
            VStack(spacing: 8) {
                Circle()
                    .fill(Color.sand200)
                    .frame(width: 64, height: 64)
                    .overlay {
                        Text(String(displayName.prefix(1)).uppercased())
                            .font(.title)
                            .foregroundStyle(.secondary)
                    }
                Text(displayName)
                    .font(.headline)
            }

            Text("Passkeys are stored securely in your password manager or iCloud, and synced across all your devices. You'll use it to sign back into this identity if you lose this device.")
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)

            if let error = errorMessage {
                Text(error)
                    .foregroundStyle(Color.avError)
                    .font(.callout)
            }

            Spacer()

            VStack(spacing: 12) {
                Button {
                    registerWithPasskey()
                } label: {
                    if isRegistering {
                        ProgressView()
                            .frame(maxWidth: .infinity)
                    } else {
                        Label("Create Passkey", systemImage: "person.badge.key")
                            .frame(maxWidth: .infinity)
                    }
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .disabled(isRegistering)

                Button {
                    // TODO: Recovery phrase generation flow
                    register(recoveryKey: Data())
                } label: {
                    Text("Use a recovery phrase instead")
                        .font(.subheadline)
                }
                .disabled(isRegistering)

                Button {
                    register(recoveryKey: Data())
                } label: {
                    Text("Skip recovery setup")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .disabled(isRegistering)
            }
            .padding(.horizontal, 32)
            .padding(.bottom, 48)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color.avPaper)
        .navigationTitle("Recovery")
        .navigationBarTitleDisplayMode(.inline)
    }

    private func registerWithPasskey() {
        isRegistering = true
        errorMessage = nil
        Task {
            do {
                // Get the window for the ASAuthorization sheet.
                guard let window = UIApplication.shared.connectedScenes
                    .compactMap({ $0 as? UIWindowScene })
                    .flatMap(\.windows)
                    .first(where: \.isKeyWindow) else {
                    errorMessage = "Could not find app window"
                    isRegistering = false
                    return
                }

                // Stage 1: pre-compute the DID locally so the passkey credential
                // can be created with the real DID as its user handle. This is
                // what makes recovery work later — the assertion returns the DID,
                // and we use it to fetch the recovery blob from the home server.
                let prepared = try await appState.prepareAccount(
                    serverUrl: inviteToken.serverUrl.absoluteString
                )

                let passkeyManager = PasskeyManager()
                let result = try await passkeyManager.register(
                    did: prepared.did(),
                    displayName: displayName,
                    anchor: window
                )

                // Stage 2: submit PLC genesis + register with the homeserver,
                // encrypting the recovery blob under the passkey's PRF-derived key.
                try await appState.finalizePreparedAccount(
                    prepared: prepared,
                    serverUrl: inviteToken.serverUrl.absoluteString,
                    serverName: inviteToken.serverName,
                    displayName: displayName,
                    recoveryKey: result.recoveryKey
                )

                if let redirect = inviteToken.postOnboardingRedirect,
                   let url = URL(string: redirect) {
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
                        appState.handleDeepLink(url)
                    }
                }
            } catch let error as ASAuthorizationError where error.code == .canceled {
                // User cancelled — don't show error, just re-enable buttons.
                isRegistering = false
            } catch {
                errorMessage = error.localizedDescription
                isRegistering = false
            }
        }
    }

    private func register(recoveryKey: Data) {
        isRegistering = true
        errorMessage = nil
        Task {
            do {
                try await appState.createAccount(
                    serverUrl: inviteToken.serverUrl.absoluteString,
                    serverName: inviteToken.serverName,
                    displayName: displayName,
                    recoveryKey: recoveryKey
                )
                // createAccount sets isOnboarding = false, which navigates to MainTabView.
                // If the invite has a post-onboarding redirect, follow it.
                if let redirect = inviteToken.postOnboardingRedirect,
                   let url = URL(string: redirect) {
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
                        appState.handleDeepLink(url)
                    }
                }
            } catch {
                errorMessage = error.localizedDescription
                isRegistering = false
            }
        }
    }
}
