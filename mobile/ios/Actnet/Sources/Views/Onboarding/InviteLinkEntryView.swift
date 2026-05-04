import SwiftUI

struct InviteLinkEntryView: View {
    @State private var linkText = ""
    @State private var inviteToken: InviteToken?
    @State private var errorMessage: String?

    var body: some View {
        VStack(spacing: 24) {
            TextField("Paste invite link", text: $linkText)
                .textFieldStyle(.roundedBorder)
                .textContentType(.URL)
                .autocapitalization(.none)
                .padding(.horizontal, 32)

            if let error = errorMessage {
                Text(error)
                    .foregroundStyle(.red)
                    .font(.callout)
            }

            Button("Continue") {
                validateLink()
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            .disabled(linkText.isEmpty)

            Spacer()
        }
        .padding(.top, 32)
        .navigationTitle("Enter Invite Link")
        .navigationBarTitleDisplayMode(.inline)
        .navigationDestination(item: $inviteToken) { token in
            IdentityPickerView(inviteToken: token)
        }
    }

    private func validateLink() {
        // TODO: Parse actnet://invite/<server>/<token> URL
        // For now, accept anything as a mock token
        guard !linkText.isEmpty else { return }
        inviteToken = InviteToken(
            serverUrl: URL(string: "https://example.org")!,
            serverName: "Example Org",
            token: "mock-token"
        )
    }
}
