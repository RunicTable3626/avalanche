import SwiftUI

/// Avatar for a contact (someone other than the local user). Shows the
/// contact's photo override if one is set; otherwise initials on a tinted
/// circle, matching `AccountAvatar`.
///
/// `imageData` is reserved for the per-contact `photo_override` field
/// (docs/35-contacts-and-profiles.md) and is `nil` until that feature lands —
/// callers can wire it through with no further change here.
struct ContactAvatar: View {
    let name: String
    var imageData: Data? = nil
    let size: CGFloat

    var body: some View {
        if let data = imageData, let uiImage = UIImage(data: data) {
            Image(uiImage: uiImage)
                .resizable()
                .scaledToFill()
                .frame(width: size, height: size)
                .clipShape(Circle())
        } else {
            let color = Color.avBrand
            Circle()
                .fill(color.opacity(0.2))
                .frame(width: size, height: size)
                .overlay {
                    Text(initial)
                        .font(.system(size: size * 0.4, weight: .medium))
                        .foregroundColor(color)
                }
        }
    }

    private var initial: String {
        let trimmed = name.trimmingCharacters(in: .whitespaces)
        return trimmed.isEmpty ? "?" : trimmed.prefix(1).uppercased()
    }
}
