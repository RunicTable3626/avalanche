import SwiftUI

enum ServiceMode: String, CaseIterable {
    case mock = "Mock (no server)"
    case devServer = "Dev Server"
}

/// Top-level app state. Tracks accounts (each backed by an AppCore instance)
/// and routes between onboarding and the main UI.
@MainActor
final class AppState: ObservableObject {
    @Published var accounts: [Account] = []
    @Published var isOnboarding: Bool = true
    @Published var conversations: [Conversation] = []
    @Published var serviceMode: ServiceMode

    /// Active AppCore instances, keyed by DID.
    private var cores: [String: any AppCoreProtocol] = [:]
    private var _service: any ActnetService

    var service: any ActnetService { _service }

    init(mode: ServiceMode = .mock) {
        self.serviceMode = mode
        self._service = Self.makeService(mode: mode)
        self.isOnboarding = true
    }

    func switchMode(_ mode: ServiceMode) {
        serviceMode = mode
        _service = Self.makeService(mode: mode)
        // Reset state on mode switch
        accounts.removeAll()
        conversations.removeAll()
        cores.removeAll()
        isOnboarding = true
    }

    private static func makeService(mode: ServiceMode) -> any ActnetService {
        switch mode {
        case .mock:
            return MockActnetService()
        case .devServer:
            return DevServerActnetService()
        }
    }

    private var dbDir: URL {
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("actnet", isDirectory: true)
    }

    func createAccount(serverUrl: String, serverName: String, displayName: String) async throws {
        let dir = dbDir
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)

        let dbPath = dir.appendingPathComponent("account-\(UUID().uuidString.prefix(8)).db").path
        // TODO: Derive key from iOS Secure Enclave instead of hardcoded passphrase
        let dbKey = "dev-placeholder-key"

        let svc = _service
        let core = try await Task.detached {
            try svc.createAccount(serverUrl: serverUrl, dbPath: dbPath, dbKey: dbKey)
        }.value

        let did = core.did()
        cores[did] = core

        let account = Account(
            id: did,
            displayName: displayName,
            avatarData: nil,
            servers: [ServerInfo(id: serverUrl, name: serverName, url: URL(string: serverUrl)!)]
        )
        accounts.append(account)

        // In mock mode, seed some fake conversations
        if serviceMode == .mock {
            conversations.append(contentsOf: MockData.seedConversations(
                accountId: did,
                serverUrl: serverUrl
            ))
        }

        isOnboarding = false
    }

    func joinServer(serverUrl: String, serverName: String, existingAccountId: String) async throws {
        if let idx = accounts.firstIndex(where: { $0.id == existingAccountId }) {
            accounts[idx].servers.append(
                ServerInfo(id: serverUrl, name: serverName, url: URL(string: serverUrl)!)
            )
        }
        isOnboarding = false
    }

    func sendMessage(conversationId: String, text: String, recipientDid: String, senderAccountId: String) async throws {
        guard let core = cores[senderAccountId] else { return }
        let plaintext = Data(text.utf8)
        try await Task.detached {
            try core.sendDm(recipientDid: recipientDid, recipientDeviceId: 1, plaintext: plaintext)
        }.value
    }

    func pollMessages(for accountId: String) async throws -> [DecryptedMessage] {
        guard let core = cores[accountId] else { return [] }
        return try await Task.detached {
            try core.receiveMessages()
        }.value
    }
}
