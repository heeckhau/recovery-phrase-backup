// Stand-ins for the Kani proofs (`cargo kani -p rpb-slip39 -Z stubbing`),
// for code a model checker can't or needn't run. Each proof that uses one
// says so in its `#[kani::stub]` attributes.

/// `zeroize::optimization_barrier` is an empty `asm!` block that only stops
/// the compiler from removing the zeroing writes. Kani has no model of
/// inline assembly, and the block changes no value, so a no-op is exact.
pub fn optimization_barrier<T: ?Sized>(_val: &T) {}

/// `getrandom::fill`, with every byte arbitrary: a proof that uses it holds
/// for every value the system RNG could return.
pub fn fill(dest: &mut [u8]) -> Result<(), getrandom::Error> {
    for byte in dest.iter_mut() {
        *byte = kani::any();
    }
    Ok(())
}

/// zeroize's `volatile_set` and `volatile_write`, which do the wiping when
/// a `Zeroizing` value drops: volatile writes to memory that is about to be
/// freed and is never read again, so skipping them changes no value a
/// proof checks. Each one otherwise costs the solver a loop per drop.
pub unsafe fn volatile_set<T: Copy + Sized>(_dst: *mut T, _src: T, _count: usize) {}

pub fn volatile_write<T: Copy + Sized>(_dst: &mut T, _src: T) {}
