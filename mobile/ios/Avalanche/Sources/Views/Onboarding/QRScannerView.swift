import SwiftUI

struct QRScannerView: View {
    @State private var scannedToken: InviteToken?

    var body: some View {
        VStack {
            // TODO: AVCaptureSession camera view for QR scanning
            Text("Camera QR Scanner")
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(Color.black.opacity(0.1))

            Text("Point your camera at an invite QR code")
                .font(.callout)
                .foregroundStyle(.secondary)
                .padding()
        }
        .navigationTitle("Scan QR Code")
        .navigationBarTitleDisplayMode(.inline)
        .navigationDestination(item: $scannedToken) { token in
            IdentityPickerView(inviteToken: token)
        }
    }
}
