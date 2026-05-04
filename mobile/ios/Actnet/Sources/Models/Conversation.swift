import Foundation

struct Conversation: Identifiable {
    let id: String
    let title: String
    let accountId: String  // which DID this conversation belongs to
    let serverUrl: String
    var lastMessage: String?
    var lastMessageDate: Date?
    var unreadCount: Int = 0
    var isGroup: Bool = false
}
