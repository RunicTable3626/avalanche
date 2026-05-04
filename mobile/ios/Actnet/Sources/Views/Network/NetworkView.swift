import SwiftUI

struct NetworkView: View {
    @EnvironmentObject var appState: AppState

    var body: some View {
        NavigationStack {
            Group {
                if allServers.isEmpty {
                    ContentUnavailableView(
                        "No servers",
                        systemImage: "server.rack",
                        description: Text("Servers and their Projects will appear here.")
                    )
                } else {
                    serverList
                }
            }
            .navigationTitle("Network")
        }
    }

    private var serverList: some View {
        List {
            ForEach(allServers) { server in
                Section(server.name) {
                    // TODO: List Projects for this server
                    Text("No Projects yet")
                        .foregroundStyle(.secondary)
                }
            }
        }
    }

    private var allServers: [ServerInfo] {
        appState.accounts.flatMap(\.servers)
            .reduce(into: [String: ServerInfo]()) { dict, server in
                dict[server.id] = server
            }
            .values
            .sorted { $0.name < $1.name }
    }
}
