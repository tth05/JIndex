thread_local! {
    static DEPTH: std::cell::Cell<u16> = const { std::cell::Cell::new(0) };
}

/// Bounds recursive parsing and snapshot reads without changing the serialized representation.
pub(crate) struct SignatureDepth;

impl SignatureDepth {
    pub(crate) fn enter() -> Option<Self> {
        DEPTH.with(|depth| {
            if depth.get() >= 256 {
                return None;
            }
            depth.set(depth.get() + 1);
            Some(Self)
        })
    }
}

impl Drop for SignatureDepth {
    fn drop(&mut self) {
        DEPTH.with(|depth| depth.set(depth.get() - 1));
    }
}
