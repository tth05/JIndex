package com.github.tth05.jindex;

import java.util.Objects;

/**
 * A bounded, deterministic page of declaration sites that reference a target.
 *
 * @param results the matching declaration sites
 * @param truncated whether additional matching sites exist after this page
 */
public record ReferenceSearchPage(ReferenceResult[] results, boolean truncated) {
    public ReferenceSearchPage {
        Objects.requireNonNull(results, "results");
        results = results.clone();
    }

    @Override
    public ReferenceResult[] results() {
        return this.results.clone();
    }
}
