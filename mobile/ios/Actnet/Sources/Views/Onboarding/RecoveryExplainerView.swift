import SwiftUI
import AuthenticationServices

struct RecoveryExplainerView: View {
    @EnvironmentObject var appState: AppState

    @State private var showRecoveryConsole = false
    @State private var showPhraseEntry = false
    @State private var errorMessage: String?
    @State private var prfOutput: Data?
    @State private var recoveryDid: String?

    var body: some View {
        VStack(spacing: 24) {
            Spacer()

            Image(systemName: "person.badge.key")
                .font(.system(size: 48))
                .foregroundStyle(Color.avBrand)

            Text("Recover an identity")
                .font(.title2)
                .fontWeight(.semibold)

            Text("Use a passkey or recovery phrase to restore an identity you created on another device.")
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)

            if let error = errorMessage {
                Text(error)
                    .foregroundStyle(Color.avError)
                    .font(.callout)
                    .padding(.horizontal, 32)
            }

            Spacer()

            VStack(spacing: 12) {
                Button {
                    recoverWithPasskey()
                } label: {
                    Label("Recover using Passkey", systemImage: "person.badge.key")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)

                Button {
                    showPhraseEntry = true
                } label: {
                    Text("Enter your recovery phrase instead")
                        .font(.subheadline)
                }
            }
            .padding(.horizontal, 32)
            .padding(.bottom, 48)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color.avPaper)
        .navigationTitle("Recovery")
        .navigationBarTitleDisplayMode(.inline)
        .navigationDestination(isPresented: $showRecoveryConsole) {
            RecoveryConsoleView(prfOutput: prfOutput ?? Data(), did: recoveryDid ?? "")
        }
        .sheet(isPresented: $showPhraseEntry) {
            RecoveryPhraseEntryView(onComplete: { phrase, serverUrl in
                recoverWithPhrase(phrase: phrase, serverUrl: serverUrl)
            })
        }
    }

    /// Validate the phrase, derive the seed (BIP39 → 32-byte seed in Rust), then
    /// recompute the DID from that seed + the user-entered home server URL — the
    /// phrase analogue of the passkey path, which recovers the URL from the
    /// credential's userHandle. On success, hands a fully-resolved DID to the
    /// console so the normal blob-download/restore path runs.
    private func recoverWithPhrase(phrase: String, serverUrl: String) {
        errorMessage = nil
        Task {
            do {
                let seed = try recoveryPhraseToSeed(phrase: phrase)
                let derivedDid = try deriveDidFromPasskey(
                    prfOutput: seed,
                    signupServerUrl: serverUrl
                )
                if appState.accounts.contains(where: { $0.id == derivedDid }) {
                    errorMessage = "This identity is already signed in on this device."
                    return
                }
                showPhraseEntry = false
                prfOutput = seed
                recoveryDid = derivedDid
                showRecoveryConsole = true
            } catch {
                errorMessage = "Invalid recovery phrase: \(error.localizedDescription)"
            }
        }
    }

    private func recoverWithPasskey() {
        errorMessage = nil
        Task {
            do {
                guard let window = UIApplication.shared.connectedScenes
                    .compactMap({ $0 as? UIWindowScene })
                    .flatMap(\.windows)
                    .first(where: \.isKeyWindow) else {
                    errorMessage = "Could not find app window"
                    return
                }

                let passkeyManager = PasskeyManager()
                let result = try await passkeyManager.authenticate(anchor: window)

                // Recompute the DID from the passkey's PRF output and the
                // signup server URL stored in the credential's userHandle.
                let derivedDid = try deriveDidFromPasskey(
                    prfOutput: result.prfOutput,
                    signupServerUrl: result.signupServerUrl
                )

                // Check if this identity is already signed in on this device.
                if appState.accounts.contains(where: { $0.id == derivedDid }) {
                    errorMessage = "This identity is already signed in on this device."
                    return
                }

                prfOutput = result.prfOutput
                recoveryDid = derivedDid
                showRecoveryConsole = true
            } catch let error as ASAuthorizationError where error.code == .canceled {
                // User cancelled — no error message needed.
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }
}

/// Sheet for entering a written-down recovery phrase plus the home server it
/// was created on. Both are needed: the phrase derives the keys, and the server
/// URL lets Rust recompute the DID (a bare phrase carries no server metadata,
/// unlike a passkey's userHandle).
private struct RecoveryPhraseEntryView: View {
    @Environment(\.dismiss) private var dismiss
    let onComplete: (_ phrase: String, _ serverUrl: String) -> Void

    @State private var phrase = ""
    @State private var serverUrl: String = {
        #if DEBUG
        return "http://localhost:3000"
        #else
        return ""
        #endif
    }()

    var body: some View {
        NavigationStack {
            VStack(spacing: 24) {
                Text("Enter your recovery phrase")
                    .font(.headline)

                TextField("Recovery phrase", text: $phrase, axis: .vertical)
                    .textFieldStyle(.roundedBorder)
                    .autocapitalization(.none)
                    .autocorrectionDisabled()
                    .padding(.horizontal, 32)
                    .lineLimit(3...6)

                VStack(alignment: .leading, spacing: 4) {
                    Text("Home server")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    TextField("https://server.example", text: $serverUrl)
                        .textFieldStyle(.roundedBorder)
                        .autocapitalization(.none)
                        .autocorrectionDisabled()
                        .keyboardType(.URL)
                }
                .padding(.horizontal, 32)

                Button("Recover") {
                    onComplete(phrase, serverUrl.trimmingCharacters(in: .whitespaces))
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .disabled(phrase.isEmpty || serverUrl.isEmpty)

                Spacer()
            }
            .padding(.top, 32)
            .navigationTitle("Recovery Phrase")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
            }
        }
    }
}
