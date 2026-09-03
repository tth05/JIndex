package com.github.tth05.jindex;

import java.util.Objects;

/**
 * A bounded, deterministic page of indexed classes.
 * This is a result limit, not cursor pagination. Simple-name searches select a
 * bounded set before ranking it by match position; a truncated set need not
 * contain the globally best-positioned matches.
 *
 * @param results matching classes in search order
 * @param truncated whether additional matching classes exist after this page
 */
public record ClassSearchPage(IndexedClass[] results, boolean truncated) {
    /**
     * Creates a search page and takes a defensive copy of its results.
     */
    public ClassSearchPage {
        Objects.requireNonNull(results, "results");
        results = results.clone();
    }

    /**
     * Returns a defensive copy of the matching classes.
     *
     * @return the matching classes in search order
     */
    @Override
    public IndexedClass[] results() {
        return this.results.clone();
    }
}
