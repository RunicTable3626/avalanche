import SwiftUI

struct RecoveryKeyBanner: View {
    @AppStorage("recoveryKeyBannerDismissed") private var dismissed = false
    @State private var showingSetup = false

    var body: some View {
        if !dismissed {
            HStack {
                Image(systemName: "shield.lefthalf.filled")
                Text("Set up your recovery key")
                    .font(.subheadline)
                Spacer()
                Button { showingSetup = true } label: {
                    Text("Set Up").font(.subheadline.bold())
                }
                Button { dismissed = true } label: {
                    Image(systemName: "xmark")
                }
            }
            .padding()
            .background(Color.yellow.opacity(0.15))
            .sheet(isPresented: $showingSetup) {
                RecoveryKeySetupView()
            }
        }
    }
}
