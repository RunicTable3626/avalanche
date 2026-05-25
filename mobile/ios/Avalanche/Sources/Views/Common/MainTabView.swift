import SwiftUI

struct MainTabView: View {
    @State private var selectedTab: Tab = .chats

    enum Tab {
        case calls, chats, network
    }

    var body: some View {
        TabView(selection: $selectedTab) {
            CallsView()
                .tabItem {
                    Label("Calls", systemImage: "phone")
                }
                .tag(Tab.calls)

            ChatsView()
                .tabItem {
                    Label("Chats", systemImage: "message")
                }
                .tag(Tab.chats)

            NetworkView()
                .tabItem {
                    Label("Network", systemImage: "server.rack")
                }
                .tag(Tab.network)
        }
    }
}
