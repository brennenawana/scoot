#if canImport(AppKit)
import AppKit

// Pure AppKit lifecycle — no SwiftUI App scene. Scoot is a menu bar citizen
// (LSUIElement); the activation policy is also set in code so bare `swift run`
// during development behaves like the bundled app. See docs/TECHNICAL.md §1.
let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.run()
#else
// Non-macOS platforms only exist so ScootCore's tests can run in Linux CI.
print("Scoot requires macOS.")
#endif
