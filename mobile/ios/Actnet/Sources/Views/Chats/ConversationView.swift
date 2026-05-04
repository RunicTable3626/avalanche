import SwiftUI

struct ConversationView: View {
    @EnvironmentObject var appState: AppState
    let conversation: Conversation

    @State private var messages: [Message] = []
    @State private var messageText = ""
    @State private var errorMessage: String?

    var body: some View {
        VStack(spacing: 0) {
            ScrollView {
                LazyVStack(spacing: 8) {
                    ForEach(messages) { message in
                        MessageBubble(
                            message: message,
                            isMe: message.senderAccountId == conversation.accountId
                        )
                    }
                }
                .padding()
            }

            if let error = errorMessage {
                Text(error)
                    .font(.caption)
                    .foregroundStyle(.red)
                    .padding(.horizontal)
            }

            Divider()

            HStack(spacing: 12) {
                TextField("Message", text: $messageText, axis: .vertical)
                    .textFieldStyle(.plain)
                    .lineLimit(1...5)

                Button {
                    sendMessage()
                } label: {
                    Image(systemName: "arrow.up.circle.fill")
                        .font(.title2)
                }
                .disabled(messageText.isEmpty)
            }
            .padding(.horizontal)
            .padding(.vertical, 8)
        }
        .navigationTitle(conversation.title)
        .navigationBarTitleDisplayMode(.inline)
    }

    private func sendMessage() {
        guard !messageText.isEmpty else { return }
        let text = messageText
        messageText = ""
        errorMessage = nil

        // Optimistically add to local UI
        let message = Message(
            id: UUID().uuidString,
            conversationId: conversation.id,
            senderAccountId: conversation.accountId,
            body: text,
            sentAt: Date()
        )
        messages.append(message)

        Task {
            do {
                // TODO: Resolve recipient DID from conversation metadata
                try await appState.sendMessage(
                    conversationId: conversation.id,
                    text: text,
                    recipientDid: "recipient-did-placeholder",
                    senderAccountId: conversation.accountId
                )
            } catch {
                errorMessage = "Failed to send: \(error.localizedDescription)"
            }
        }
    }
}
