import SwiftUI
import AuthenticationServices
import CryptoKit

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
            RecoveryPhraseEntryView(onComplete: { phrase in
                showPhraseEntry = false
                // Treat the phrase as a 32-byte seed by hashing it. Rust's
                // HKDF expects a 32-byte input; the hash plays the same role
                // a passkey PRF output would.
                let phraseData = Data(phrase.utf8)
                let hash = SHA256Hash.hash(data: phraseData)
                prfOutput = Data(hash)
                // Phrase recovery doesn't know the signup server URL; the
                // console prompts for it and asks Rust to recompute the DID.
                recoveryDid = ""
                showRecoveryConsole = true
            })
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

/// Simple SHA-256 wrapper.
private enum SHA256Hash {
    static func hash(data: Data) -> [UInt8] {
        let digest = CryptoKit.SHA256.hash(data: data)
        return Array(digest)
    }
}

/// Sheet for entering a written-down recovery phrase.
private struct RecoveryPhraseEntryView: View {
    @Environment(\.dismiss) private var dismiss
    let onComplete: (String) -> Void

    @State private var phrase = ""

    var body: some View {
        NavigationStack {
            VStack(spacing: 24) {
                Text("Enter your recovery phrase")
                    .font(.headline)

                TextField("Recovery phrase", text: $phrase, axis: .vertical)
                    .textFieldStyle(.roundedBorder)
                    .padding(.horizontal, 32)
                    .lineLimit(3...6)

                Button("Recover") {
                    onComplete(phrase)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .disabled(phrase.isEmpty)

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
