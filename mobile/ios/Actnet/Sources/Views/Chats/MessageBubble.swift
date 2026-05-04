import SwiftUI

struct MessageBubble: View {
    let message: Message
    let isMe: Bool

    var body: some View {
        HStack {
            if isMe { Spacer(minLength: 60) }

            VStack(alignment: isMe ? .trailing : .leading, spacing: 4) {
                Text(message.body)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                    .background(isMe ? Color.accentColor : Color(.systemGray5))
                    .foregroundStyle(isMe ? .white : .primary)
                    .clipShape(RoundedRectangle(cornerRadius: 16))

                HStack(spacing: 4) {
                    Text(message.sentAt, style: .time)
                    if message.isEdited {
                        Text("· Edited")
                    }
                }
                .font(.caption2)
                .foregroundStyle(.secondary)
            }

            if !isMe { Spacer(minLength: 60) }
        }
    }
}
