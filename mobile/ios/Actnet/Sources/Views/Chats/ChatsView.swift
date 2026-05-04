import SwiftUI

struct ChatsView: View {
    @EnvironmentObject var appState: AppState
    @State private var showDevSettings = false

    var body: some View {
        NavigationStack {
            Group {
                if appState.conversations.isEmpty {
                    ContentUnavailableView(
                        "No conversations yet",
                        systemImage: "message",
                        description: Text("Messages from all your servers will appear here.")
                    )
                } else {
                    conversationList
                }
            }
            .navigationTitle("Chats")
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button {
                        showDevSettings = true
                    } label: {
                        Image(systemName: "gearshape")
                            .font(.subheadline)
                    }
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        // TODO: New message compose
                    } label: {
                        Image(systemName: "square.and.pencil")
                    }
                }
            }
            .overlay(alignment: .top) {
                if !hasRecoveryKey {
                    RecoveryKeyBanner()
                }
            }
            .sheet(isPresented: $showDevSettings) {
                DevSettingsView()
            }
        }
    }

    private var conversationList: some View {
        List(appState.conversations) { conversation in
            NavigationLink {
                ConversationView(conversation: conversation)
            } label: {
                ConversationRow(conversation: conversation, account: accountFor(conversation))
            }
        }
        .listStyle(.plain)
    }

    private var hasRecoveryKey: Bool {
        // TODO: Check via Rust core
        false
    }

    private func accountFor(_ conversation: Conversation) -> Account? {
        appState.accounts.first { $0.id == conversation.accountId }
    }
}
