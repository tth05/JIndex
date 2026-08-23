package com.github.tth05.jindex;

import java.util.Objects;

/**
 * A bounded, deterministic page of exact Java string values.
 *
 * @param values the matching string values
 * @param truncated whether additional matching values exist after this page
 */
public record LiteralSearchPage(String[] values, boolean truncated) {
    public LiteralSearchPage {
        Objects.requireNonNull(values, "values");
        values = values.clone();
    }

    @Override
    public String[] values() {
        return this.values.clone();
    }
}
