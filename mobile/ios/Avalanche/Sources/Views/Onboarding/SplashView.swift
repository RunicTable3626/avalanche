import SwiftUI

struct SplashView: View {
    @State private var showScanner = false
    @State private var showLinkEntry = false
    @State private var showDevSettings = false

    var body: some View {
        NavigationStack {
            VStack(spacing: 32) {
                Spacer()

                Text("Avalanche")
                    .font(.largeTitle)
                    .fontWeight(.bold)

                Text("Encrypted organizing")
                    .font(.title3)
                    .foregroundStyle(.secondary)

                Spacer()

                VStack(spacing: 16) {
                    Button {
                        showScanner = true
                    } label: {
                        Label("Scan Invite QR Code", systemImage: "qrcode.viewfinder")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.large)

                    Button {
                        showLinkEntry = true
                    } label: {
                        Label("Enter Invite Link", systemImage: "link")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.large)
                }
                .padding(.horizontal, 32)
                .padding(.bottom, 48)
            }
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        showDevSettings = true
                    } label: {
                        Image(systemName: "gearshape")
                            .font(.subheadline)
                    }
                }
            }
            .navigationDestination(isPresented: $showScanner) {
                QRScannerView()
            }
            .navigationDestination(isPresented: $showLinkEntry) {
                InviteLinkEntryView()
            }
            .sheet(isPresented: $showDevSettings) {
                DevSettingsView()
            }
        }
    }
}
