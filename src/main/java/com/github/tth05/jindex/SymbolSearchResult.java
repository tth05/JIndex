package com.github.tth05.jindex;

import java.util.Objects;

public record SymbolSearchResult(
        long symbolId,
        SymbolKind kind,
        String ownerInternalName,
        String name,
        String descriptor,
        int sourceId,
        int accessFlags
) {
    SymbolSearchResult(
            long symbolId,
            int kind,
            String ownerInternalName,
            String name,
            String descriptor,
            int sourceId,
            int accessFlags
    ) {
        this(
                symbolId,
                SymbolKind.values()[kind],
                Objects.requireNonNull(ownerInternalName, "ownerInternalName"),
                Objects.requireNonNull(name, "name"),
                Objects.requireNonNull(descriptor, "descriptor"),
                sourceId,
                accessFlags
        );
    }
}
