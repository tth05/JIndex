use crate::semantic_index::SymbolKind;

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub(crate) enum ClassReferenceKind {
    Hierarchy = 1 << 0,
    Declaration = 1 << 1,
    AnnotationOrMetadata = 1 << 2,
    RuntimeType = 1 << 3,
    MemberUsage = 1 << 4,
}

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub(crate) enum FieldReferenceKind {
    Read = 1 << 0,
    Write = 1 << 1,
    Handle = 1 << 2,
}

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub(crate) enum MethodReferenceKind {
    Invoke = 1 << 0,
    Handle = 1 << 1,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum MemberReferenceKind {
    Field(FieldReferenceKind),
    Method(MethodReferenceKind),
}

impl ClassReferenceKind {
    pub(crate) const fn mask(self) -> u8 {
        self as u8
    }
}

impl FieldReferenceKind {
    pub(crate) const fn mask(self) -> u8 {
        self as u8
    }
}

impl MethodReferenceKind {
    pub(crate) const fn mask(self) -> u8 {
        self as u8
    }
}

impl MemberReferenceKind {
    pub(crate) const fn mask(self) -> u8 {
        match self {
            Self::Field(kind) => kind.mask(),
            Self::Method(kind) => kind.mask(),
        }
    }
}

// These public-mask positions mirror ReferenceKind.java's explicit mask constants. The persisted target-specific mask remains
// compact, while JNI expands it into this target-independent API representation.
pub(crate) fn public_mask(target_kind: SymbolKind, stored_mask: u8) -> u16 {
    let base = match target_kind {
        SymbolKind::Class => 0,
        SymbolKind::Field => 5,
        SymbolKind::Method => 8,
    };
    u16::from(stored_mask) << base
}

pub(crate) const STRING_LITERAL_PUBLIC_MASK: u16 = 1 << 10;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_specific_masks_expand_to_the_public_api_positions() {
        assert_eq!(
            public_mask(
                SymbolKind::Class,
                ClassReferenceKind::Hierarchy.mask() | ClassReferenceKind::MemberUsage.mask()
            ),
            (1 << 0) | (1 << 4)
        );
        assert_eq!(
            public_mask(
                SymbolKind::Field,
                FieldReferenceKind::Read.mask() | FieldReferenceKind::Write.mask()
            ),
            (1 << 5) | (1 << 6)
        );
        assert_eq!(
            public_mask(SymbolKind::Method, MethodReferenceKind::Handle.mask()),
            1 << 9
        );
        assert_eq!(STRING_LITERAL_PUBLIC_MASK, 1 << 10);
    }
}
