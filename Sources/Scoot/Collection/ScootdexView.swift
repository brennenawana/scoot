#if canImport(AppKit)
import SwiftUI
import ScootCore

/// The collection window (design/surfaces/scootdex): native chrome outside,
/// pixel shelf inside. Owned cells are alive (slow idle animation); unpulled
/// species are silhouettes + "???"; the Secret shows nothing at all until
/// pulled. No guilt surfaces: nothing here is missed, late, or expiring.
struct ScootdexView: View {
    @ObservedObject var manager: CollectionManager
    /// Replays an owned buddy's reveal — pure theater, no state.
    var onReplay: ((Buddy) -> Void)? = nil
    @State private var selectedID: String?

    /// Catalog order with secrets last — whispers sit at the end of the shelf.
    private var shelf: [Buddy] {
        manager.catalog.species.filter { $0.rarity != .secret }
            + manager.catalog.species.filter { $0.rarity == .secret }
    }

    private var selected: Buddy? {
        let id = selectedID ?? manager.activeSpecies?.id
        return id.flatMap { manager.catalog.species(withID: $0) }
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack(alignment: .top, spacing: 0) {
                shelfColumn
                    .frame(width: 392)
                Divider()
                detailColumn
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            Divider()
            footer
        }
        .frame(width: 640, height: 430)
    }

    // MARK: - Shelf

    private var shelfColumn: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .firstTextBaseline) {
                Text("\(manager.foundCount) of \(manager.catalog.species.count) found")
                    .font(.system(size: 13, weight: .semibold))
                Text("\(Int((Double(manager.foundCount) / Double(manager.catalog.species.count) * 100).rounded()))%")
                    .font(.system(size: 11))
                    .foregroundStyle(.secondary)
                Spacer()
                Text("✦ \(manager.state.sparks) sparks")
                    .font(.system(size: 11))
                    .foregroundStyle(.secondary)
            }
            .padding(.horizontal, 2)

            LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 8), count: 4),
                      spacing: 8) {
                ForEach(shelf) { species in
                    cell(for: species)
                }
            }
            Spacer(minLength: 0)
        }
        .padding(14)
    }

    @ViewBuilder
    private func cell(for species: Buddy) -> some View {
        let ownedIndex = manager.state.ownedIndex(of: species.id)
        let isActive = manager.activeSpecies?.id == species.id
        let isSelected = selected?.id == species.id
        VStack(spacing: 4) {
            ZStack {
                if let index = ownedIndex, let owned = manager.state.owned[safe: index],
                   let sheet = SpriteLibrary.sheet(for: species) {
                    BuddyView(sheet: sheet, fps: 2, scale: 1)
                        .accessibilityLabel("\(owned.givenName), \(species.displayName)")
                } else if species.rarity == .secret {
                    Text("?")
                        .font(.system(size: 16, design: .monospaced))
                        .foregroundStyle(.secondary.opacity(0.6))
                } else if let silhouette = SpriteLibrary.silhouette(for: species) {
                    Image(nsImage: silhouette)
                        .resizable()
                        .interpolation(.none)
                        .frame(width: 32, height: 32)
                        .opacity(0.3)
                }
            }
            .frame(height: 36)

            if let index = ownedIndex, let owned = manager.state.owned[safe: index] {
                Text(owned.givenName.uppercased())
                    .font(.system(size: 9.5, design: .monospaced))
                    .tracking(0.5)
                    .lineLimit(1)
                HStack(spacing: 3) {
                    Circle().fill(species.rarity.color).frame(width: 6, height: 6)
                    Text(species.displayName)
                }
                .font(.system(size: 9))
                .foregroundStyle(.secondary)
                .lineLimit(1)
            } else if species.rarity == .secret {
                Text("· · ·")
                    .font(.system(size: 9.5, design: .monospaced))
                    .foregroundStyle(.secondary.opacity(0.6))
                Text(" ").font(.system(size: 9))
            } else {
                Text("???")
                    .font(.system(size: 9.5, design: .monospaced))
                    .foregroundStyle(.secondary)
                HStack(spacing: 3) {
                    Circle().fill(species.rarity.color).frame(width: 6, height: 6)
                    Text(species.rarity.displayName)
                }
                .font(.system(size: 9))
                .foregroundStyle(.secondary)
            }
        }
        .padding(.vertical, 8)
        .padding(.horizontal, 4)
        .frame(maxWidth: .infinity)
        // Selection = tinted fill (instant click feedback); on duty = accent
        // border. Distinct, so browsing never hides who's performing.
        .background(isSelected ? Color.accentColor.opacity(0.12) : Color.primary.opacity(0.05),
                    in: RoundedRectangle(cornerRadius: 9))
        .overlay(
            RoundedRectangle(cornerRadius: 9)
                .strokeBorder(isActive ? Color.accentColor : .clear, lineWidth: 1)
        )
        .contentShape(Rectangle())
        .onTapGesture { selectedID = species.id }
    }

    // MARK: - Detail

    @ViewBuilder
    private var detailColumn: some View {
        if let species = selected {
            if let index = manager.state.ownedIndex(of: species.id) {
                ownedDetail(species: species, index: index)
            } else {
                unownedDetail(species: species)
            }
        } else {
            Text("Pick a slot")
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    private func ownedDetail(species: Buddy, index: Int) -> some View {
        let owned = manager.state.owned[safe: index]
        return VStack(spacing: 10) {
            ZStack {
                RadialGradient(colors: [species.rarity.color.opacity(0.18), .clear],
                               center: .center, startRadius: 5, endRadius: 70)
                if let sheet = SpriteLibrary.sheet(for: species) {
                    PortraitFlourishView(sheet: sheet,
                                         celebrateSheet: SpriteLibrary.celebrateSheet(for: species),
                                         bondScoots: owned?.bondScoots ?? 0)
                        .id(species.id)
                }
            }
            .frame(width: 130, height: 96)

            // .id ties the field's local state to THIS buddy. Without it,
            // SwiftUI keeps the previous buddy's draft across a selection
            // change (same structural position), and a focus-loss commit
            // could write buddy A's name onto buddy B.
            NameField(name: owned?.givenName ?? "", onCommit: { manager.rename(at: index, to: $0) })
                .id(species.id)

            HStack(spacing: 5) {
                Text(species.displayName)
                    .font(.system(size: 11))
                    .foregroundStyle(.secondary)
                species.rarity.badge
            }

            Text(species.flavor)
                .font(.system(size: 10.5))
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 12)

            Text("Bond · \(owned?.bondScoots ?? 0) \(owned?.bondScoots == 1 ? "scoot" : "scoots") together")
                .font(.system(size: 11))
                .foregroundStyle(.secondary)

            if manager.state.activeBuddyIndex == index {
                Label("On duty", systemImage: "checkmark")
                    .font(.system(size: 11.5, weight: .semibold))
                    .foregroundStyle(Color.accentColor)
            } else {
                Button("Put on duty") { manager.setActive(index: index) }
                    .controlSize(.small)
            }
            if let onReplay {
                Button("Replay reveal") { onReplay(species) }
                    .buttonStyle(.plain)
                    .font(.system(size: 10.5))
                    .foregroundStyle(.secondary)
            }
            Spacer(minLength: 0)
        }
        .padding(14)
    }

    private func unownedDetail(species: Buddy) -> some View {
        VStack(spacing: 10) {
            ZStack {
                if species.rarity == .secret {
                    Text("?")
                        .font(.system(size: 34, design: .monospaced))
                        .foregroundStyle(.secondary.opacity(0.5))
                } else if let silhouette = SpriteLibrary.silhouette(for: species) {
                    Image(nsImage: silhouette)
                        .resizable()
                        .interpolation(.none)
                        .frame(width: 64, height: 64)
                        .opacity(0.3)
                }
            }
            .frame(width: 130, height: 96)

            Text(species.rarity == .secret ? "· · ·" : "???")
                .font(.system(size: 13, design: .monospaced))
                .foregroundStyle(.secondary)

            if species.rarity != .secret {
                HStack(spacing: 5) {
                    Circle().fill(species.rarity.color).frame(width: 6, height: 6)
                    Text(species.rarity.displayName)
                }
                .font(.system(size: 11))
                .foregroundStyle(.secondary)
            }

            Text("Keep scooting.")
                .font(.system(size: 11))
                .foregroundStyle(.secondary)
            Spacer(minLength: 0)
        }
        .padding(14)
    }

    private var footer: some View {
        HStack {
            Text("Roll odds: \(RarityTier.disclosure)")
            Spacer()
            Text("Duplicates become sparks")
        }
        .font(.system(size: 10.5))
        .foregroundStyle(.secondary)
        .padding(.horizontal, 14)
        .padding(.vertical, 8)
    }
}

/// Inline, always-editable name field — renaming is free and instant
/// (docs/PRODUCT.md §2). Commits on submit or focus loss.
private struct NameField: View {
    @State var name: String
    let onCommit: (String) -> Void
    @FocusState private var focused: Bool

    var body: some View {
        TextField("Name", text: $name)
            .textFieldStyle(.plain)
            .font(.system(size: 13, weight: .semibold, design: .monospaced))
            .multilineTextAlignment(.center)
            .frame(width: 150)
            .focused($focused)
            .onSubmit { onCommit(name) }
            .onChange(of: focused) { _, isFocused in
                if !isFocused { onCommit(name) }
            }
    }
}

extension Array {
    subscript(safe index: Int) -> Element? {
        indices.contains(index) ? self[index] : nil
    }
}
#endif
