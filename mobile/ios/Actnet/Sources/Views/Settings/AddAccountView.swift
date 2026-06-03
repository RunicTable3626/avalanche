import SwiftUI

struct AddAccountView: View {
    @State private var showScanner = false
    @State private var showLinkEntry = false
    @State private var showRecovery = false

    var body: some View {
        VStack(spacing: 16) {
            Spacer()

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

            Button {
                showRecovery = true
            } label: {
                Text("Recover a different identity")
                    .font(.subheadline)
            }
            .padding(.top, 8)

            Spacer()
        }
        .padding(.horizontal, 32)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color.avPaper)
        .navigationTitle("Add Account")
        .navigationBarTitleDisplayMode(.inline)
        .navigationDestination(isPresented: $showScanner) {
            QRScannerView()
        }
        .navigationDestination(isPresented: $showLinkEntry) {
            InviteLinkEntryView()
        }
        .navigationDestination(isPresented: $showRecovery) {
            RecoveryExplainerView()
        }
    }
}
