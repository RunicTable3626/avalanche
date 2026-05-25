import Foundation
import Combine

struct LogEntry: Identifiable {
    let id = UUID()
    let timestamp: Date
    let category: String
    let message: String
    let level: Level

    enum Level: String {
        case info, warn, error, ok
    }

    var formatted: String {
        let f = LogEntry.formatter.string(from: timestamp)
        return "\(f) [\(category)] \(message)"
    }

    private static let formatter: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "HH:mm:ss.SSS"
        return f
    }()
}

/// In-memory ring buffer of log entries, observable from SwiftUI.
/// Thread-safe: writes funnel onto the main actor.
@MainActor
final class AppLog: ObservableObject {
    static let shared = AppLog()

    private static let capacity = 1000

    @Published private(set) var entries: [LogEntry] = []

    private init() {}

    static nonisolated func info(_ category: String, _ message: String) {
        append(.info, category, message)
    }
    static nonisolated func warn(_ category: String, _ message: String) {
        append(.warn, category, message)
    }
    static nonisolated func error(_ category: String, _ message: String) {
        append(.error, category, message)
    }
    static nonisolated func ok(_ category: String, _ message: String) {
        append(.ok, category, message)
    }

    private static nonisolated func append(_ level: LogEntry.Level, _ category: String, _ message: String) {
        let entry = LogEntry(timestamp: Date(), category: category, message: message, level: level)
        print("[\(category)] \(message)")
        Task { @MainActor in
            let log = AppLog.shared
            log.entries.append(entry)
            if log.entries.count > AppLog.capacity {
                log.entries.removeFirst(log.entries.count - AppLog.capacity)
            }
        }
    }

    func clear() {
        entries.removeAll()
    }
}
