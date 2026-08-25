package com.github.tth05.jindex;

import java.util.Objects;

/**
 * Counts needed to annotate one indexed class and its declared methods.
 * Method arrays use the same order as {@link IndexedClass#getMethods()}.
 *
 * @param implementationCount number of indexed subclasses and implementors
 * @param methodImplementationCounts implementation counts for each declared method
 * @param methodBaseCounts base-declaration counts for each declared method
 */
public record HierarchySummary(
        int implementationCount,
        int[] methodImplementationCounts,
        int[] methodBaseCounts
) {
    public HierarchySummary {
        if (implementationCount < 0) {
            throw new IllegalArgumentException("Implementation count must not be negative");
        }
        Objects.requireNonNull(methodImplementationCounts, "methodImplementationCounts");
        Objects.requireNonNull(methodBaseCounts, "methodBaseCounts");
        if (methodImplementationCounts.length != methodBaseCounts.length) {
            throw new IllegalArgumentException("Method hierarchy count arrays have different lengths");
        }
        methodImplementationCounts = methodImplementationCounts.clone();
        methodBaseCounts = methodBaseCounts.clone();
        for (int count : methodImplementationCounts) {
            if (count < 0) {
                throw new IllegalArgumentException("Method implementation count must not be negative");
            }
        }
        for (int count : methodBaseCounts) {
            if (count < 0) {
                throw new IllegalArgumentException("Method base count must not be negative");
            }
        }
    }

    @Override
    public int[] methodImplementationCounts() {
        return this.methodImplementationCounts.clone();
    }

    @Override
    public int[] methodBaseCounts() {
        return this.methodBaseCounts.clone();
    }
}
