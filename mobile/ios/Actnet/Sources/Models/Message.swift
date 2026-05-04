import Foundation

struct Message: Identifiable {
    let id: String
    let conversationId: String
    let senderAccountId: String
    let body: String
    let sentAt: Date
    var editedAt: Date?
    var isEdited: Bool { editedAt != nil }
}
