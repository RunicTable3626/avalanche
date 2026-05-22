import SwiftUI

struct RecoveryKeySetupView: View {
    @EnvironmentObject var appState: AppState
    @Environment(\.dismiss) private var dismiss
    @State private var keyHex: String?
    @State private var errorMessage: String?
    @State private var confirmed = false

    var body: some View {
        NavigationView {
            VStack(spacing: 24) {
                if let keyHex {
                    Text("Write down or copy this key. It cannot be recovered if lost.")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                    Text(keyHex)
                        .font(.system(.body, design: .monospaced))
                        .textSelection(.enabled)
                        .padding()
                        .background(Color(.secondarySystemBackground), in: RoundedRectangle(cornerRadius: 8))
                    Button("I've saved my recovery key") {
                        confirmed = true
                        dismiss()
                    }
                    .buttonStyle(.borderedProminent)
                } else if let errorMessage {
                    Text(errorMessage).foregroundStyle(.red)
                } else {
                    ProgressView()
                }
            }
            .padding()
            .navigationTitle("Recovery Key")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
            }
            .task {
                do {
                    let keyData = try await Task.detached {
                        guard let core = appState.cores.first?.value else {
                            throw NSError(domain: "Actnet", code: 1, userInfo: [NSLocalizedDescriptionKey: "No account"])
                        }
                        return try core.generateRecoveryKey()
                    }.value
                    keyHex = keyData.map { String(format: "%02x", $0) }.joined(separator: " ")
                    await appState.refreshRecoveryKeyStatus()
                } catch {
                    errorMessage = error.localizedDescription
                }
            }
        }
    }
}

#Preview {
    RecoveryKeySetupView()
        .environmentObject(AppState())
}
