import Foundation

/// In-memory view of a conversation as the chat list sees it.
///
/// The persistent source of truth is `message_history` in SQLCipher; this
/// struct is rebuilt from `AppCore.loadConversations()` on startup and kept
/// in sync from in-session message activity. Nothing in this struct is
/// persisted to UserDefaults — plaintext bodies and DIDs would leak there.
struct Conversation: Identifiable, Hashable {
    let id: String
    var title: String
    let accountId: String  // which DID this conversation belongs to
    let serverUrl: String
    var recipientDid: String?  // for DMs: the other party's DID
    var lastMessage: String?
    var lastMessageDate: Date?
    var isGroup: Bool = false
}
