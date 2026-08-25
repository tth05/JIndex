package com.github.tth05.jindex;

import java.util.Arrays;
import java.util.Objects;

/**
 * A distinct indexed Java string value and the source IDs in which it occurs.
 *
 * @param value exact Java string value
 * @param sourceIds sorted, unique source IDs containing the value
 */
public record LiteralSearchResult(String value, int[] sourceIds) {
    public LiteralSearchResult {
        Objects.requireNonNull(value, "value");
        Objects.requireNonNull(sourceIds, "sourceIds");
        sourceIds = sourceIds.clone();
        for (int sourceId : sourceIds) {
            if (sourceId < 0) {
                throw new IllegalArgumentException("sourceIds must not contain negative values");
            }
        }
        Arrays.sort(sourceIds);
        for (int index = 1; index < sourceIds.length; index++) {
            if (sourceIds[index - 1] == sourceIds[index]) {
                throw new IllegalArgumentException("sourceIds must be unique");
            }
        }
    }

    @Override
    public int[] sourceIds() {
        return this.sourceIds.clone();
    }
}
