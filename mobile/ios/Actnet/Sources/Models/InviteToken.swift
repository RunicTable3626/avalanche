import Foundation

struct InviteToken: Identifiable, Hashable {
    var id: String { token }
    let serverUrl: URL
    let serverName: String
    let token: String
}
