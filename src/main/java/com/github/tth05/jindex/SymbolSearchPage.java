package com.github.tth05.jindex;

import java.util.Objects;

/**
 * Bounded symbol results with explicit truncation metadata, not cursor pagination.
 *
 * @param results matching declarations in deterministic search order
 * @param truncated whether the limit omitted additional matching declarations
 */
public record SymbolSearchPage(SymbolSearchResult[] results, boolean truncated) {
    /** Creates a page with a defensive copy of its results. */
    public SymbolSearchPage {
        Objects.requireNonNull(results, "results");
        results = results.clone();
    }

    /** @return a defensive copy of the matching declarations */
    @Override
    public SymbolSearchResult[] results() {
        return this.results.clone();
    }
}
