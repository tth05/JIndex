package com.github.tth05.jindex;

import java.util.Objects;

/**
 * A bounded, deterministic page of exact Java string values.
 *
 * @param results the matching string values and the sources in which each value occurs
 * @param truncated whether additional matching values exist after this page
 */
public record LiteralSearchPage(LiteralSearchResult[] results, boolean truncated) {
    public LiteralSearchPage {
        Objects.requireNonNull(results, "results");
        results = results.clone();
    }

    @Override
    public LiteralSearchResult[] results() {
        return this.results.clone();
    }
}
