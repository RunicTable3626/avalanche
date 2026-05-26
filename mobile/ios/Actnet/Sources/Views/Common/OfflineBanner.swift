import SwiftUI

/// Floating pill shown when any account's connection to its homeserver is
/// not in the `.connected` state. Drives entirely off
/// `AppState.aggregateConnectionState` — the Rust reconnect task is the
/// source of truth.
struct OfflineBanner: View {
    @EnvironmentObject var appState: AppState

    var body: some View {
        let state = appState.aggregateConnectionState
        if shouldShow(state) {
            // TimelineView re-evaluates its body on its own schedule and passes
            // a fresh `context.date`. We use that — not a @State Date — so the
            // countdown never lags behind real time or skips values when the
            // surrounding view re-renders for other reasons.
            TimelineView(.periodic(from: .now, by: 1.0)) { context in
                HStack(spacing: 8) {
                    ProgressView()
                        .controlSize(.small)
                        .tint(.white)
                    Text(statusText(for: state, now: context.date))
                        .font(.footnote.weight(.medium))
                        .foregroundStyle(.white)
                        .lineLimit(1)
                }
                .padding(.horizontal, 14)
                .padding(.vertical, 8)
                .background(
                    Capsule()
                        .fill(Color(red: 0.78, green: 0.42, blue: 0.10))
                )
                .shadow(color: .black.opacity(0.15), radius: 6, y: 2)
                .padding(.top, 4)
                .transition(.move(edge: .top).combined(with: .opacity))
            }
        }
    }

    private func shouldShow(_ state: ConnectionState) -> Bool {
        if case .connected = state { return false }
        return true
    }

    private func statusText(for state: ConnectionState, now: Date) -> String {
        switch state {
        case .connected:
            return ""
        case .connecting, .disconnected:
            return "Reconnecting…"
        case .reconnecting(let nextAttemptAtMs):
            let nowMs = Int64(now.timeIntervalSince1970 * 1000)
            let diffMs = nextAttemptAtMs - nowMs
            let secs = max(0, Int((diffMs + 999) / 1000))
            if secs <= 0 {
                return "Reconnecting…"
            }
            return "Offline · retrying in \(secs)s"
        }
    }
}
