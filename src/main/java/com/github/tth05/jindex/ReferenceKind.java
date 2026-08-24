package com.github.tth05.jindex;

import java.util.EnumSet;
import java.util.Set;

/** The semantic relationship between a usage site and its target. */
public enum ReferenceKind {
    CLASS_HIERARCHY(1 << 0),
    CLASS_DECLARATION(1 << 1),
    CLASS_ANNOTATION_OR_METADATA(1 << 2),
    CLASS_RUNTIME_TYPE(1 << 3),
    CLASS_MEMBER_USAGE(1 << 4),
    FIELD_READ(1 << 5),
    FIELD_WRITE(1 << 6),
    FIELD_HANDLE(1 << 7),
    METHOD_INVOKE(1 << 8),
    METHOD_HANDLE(1 << 9),
    STRING_LITERAL(1 << 10);

    private static final int KNOWN_MASK = (1 << values().length) - 1;
    private final int mask;

    ReferenceKind(int mask) {
        this.mask = mask;
    }

    static Set<ReferenceKind> fromMask(int mask) {
        EnumSet<ReferenceKind> kinds = EnumSet.noneOf(ReferenceKind.class);
        if ((mask & ~KNOWN_MASK) != 0) {
            throw new IllegalArgumentException("Unknown reference-kind mask 0x" + Integer.toHexString(mask));
        }
        for (ReferenceKind kind : values()) {
            if ((mask & kind.mask) != 0) {
                kinds.add(kind);
            }
        }
        if (kinds.isEmpty()) {
            throw new IllegalArgumentException("A reference must have at least one kind");
        }
        return Set.copyOf(kinds);
    }
}
