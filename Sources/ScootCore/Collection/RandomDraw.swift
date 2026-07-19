import Foundation

/// Scoot's own bounded uniform draw — rejection-modulo over the raw 64-bit
/// stream. Owned (rather than `Int.random(in:using:)`) because roll behavior
/// is a cross-platform contract (docs/CONTRACTS.md): every port must produce
/// identical sequences from identical seeds, and Swift's stdlib algorithm is
/// an implementation detail we must not depend on.
///
/// Spec (identical in every implementation):
///   k = (2^64 mod bound)              // computed as ((0 &- b) % b) in 64-bit
///   loop:
///     r = next()                      // raw generator output
///     if k == 0 || r <= UInt64.max - k { return r % b }   // unbiased region
///     // else discard r and redraw
public enum RandomDraw {
    public static func uniform<R: RandomNumberGenerator>(_ bound: Int, using rng: inout R) -> Int {
        precondition(bound > 0, "bound must be positive")
        let b = UInt64(bound)
        let k = (0 &- b) % b // 2^64 mod b
        while true {
            let r = rng.next()
            if k == 0 || r <= UInt64.max - k {
                return Int(r % b)
            }
        }
    }
}
