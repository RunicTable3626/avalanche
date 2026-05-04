import Foundation

struct Account: Identifiable, Hashable {
    let id: String  // DID
    var displayName: String
    var avatarData: Data?
    var servers: [ServerInfo]
}

struct ServerInfo: Identifiable, Hashable {
    let id: String  // server URL
    let name: String
    let url: URL
}
