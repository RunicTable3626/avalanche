import Foundation

/// Abstraction over the Rust AppCore for account creation/login.
/// The returned AppCoreProtocol instance handles all subsequent operations
/// (send, receive, etc.) matching the UniFFI-generated interface.
protocol ActnetService: Sendable {
    func createAccount(serverUrl: String, dbPath: String, dbKey: String) throws -> any AppCoreProtocol
    func login(dbPath: String, dbKey: String) throws -> any AppCoreProtocol
}

/// Uses the real UniFFI-generated AppCore bindings.
/// Uncomment when the Rust XCFramework is linked.
// struct RealActnetService: ActnetService {
//     func createAccount(serverUrl: String, dbPath: String, dbKey: String) throws -> any AppCoreProtocol {
//         try AppCore.createAccount(serverUrl: serverUrl, dbPath: dbPath, dbKey: dbKey)
//     }
//     func login(dbPath: String, dbKey: String) throws -> any AppCoreProtocol {
//         try AppCore.login(dbPath: dbPath, dbKey: dbKey)
//     }
// }
