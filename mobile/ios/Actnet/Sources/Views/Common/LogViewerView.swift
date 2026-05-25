import SwiftUI
import UIKit

/// General-purpose log viewer. Reads from `AppLog.shared` and shows newest
/// entries at the bottom. Accessible via a hidden two-finger triple-tap
/// gesture from anywhere in the app.
struct LogViewerView: View {
    @ObservedObject private var log = AppLog.shared
    @Environment(\.dismiss) private var dismiss
    @State private var filter: String = ""

    private var visible: [LogEntry] {
        guard !filter.isEmpty else { return log.entries }
        let lower = filter.lowercased()
        return log.entries.filter {
            $0.message.lowercased().contains(lower) || $0.category.lowercased().contains(lower)
        }
    }

    var body: some View {
        NavigationStack {
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 2) {
                        ForEach(visible) { entry in
                            Text(entry.formatted)
                                .font(.system(.caption2, design: .monospaced))
                                .foregroundStyle(color(for: entry.level))
                                .textSelection(.enabled)
                                .id(entry.id)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(8)
                }
                .onAppear {
                    if let last = visible.last { proxy.scrollTo(last.id, anchor: .bottom) }
                }
                .onChange(of: visible.count) { _, _ in
                    if let last = visible.last { proxy.scrollTo(last.id, anchor: .bottom) }
                }
            }
            .background(Color.avPaper)
            .searchable(text: $filter, placement: .navigationBarDrawer(displayMode: .always))
            .navigationTitle("Logs")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("Close") { dismiss() }
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Menu {
                        Button("Copy all") { copyAll() }
                        Button("Clear", role: .destructive) { log.clear() }
                    } label: {
                        Image(systemName: "ellipsis.circle")
                    }
                }
            }
        }
    }

    private func color(for level: LogEntry.Level) -> Color {
        switch level {
        case .info: return .primary
        case .warn: return .orange
        case .error: return Color.avError
        case .ok: return Color.avBrand
        }
    }

    private func copyAll() {
        UIPasteboard.general.string = visible.map { $0.formatted }.joined(separator: "\n")
    }
}
