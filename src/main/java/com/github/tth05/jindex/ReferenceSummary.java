package com.github.tth05.jindex;

/**
 * Aggregate incoming-reference counts for one exact JVM declaration.
 *
 * @param siteCount number of distinct containing declarations
 * @param occurrenceCount total bytecode occurrences across those declarations
 */
public record ReferenceSummary(long siteCount, long occurrenceCount) {
    public ReferenceSummary {
        if (siteCount < 0 || occurrenceCount < 0) {
            throw new IllegalArgumentException("Reference counts must not be negative");
        }
        if (siteCount > occurrenceCount) {
            throw new IllegalArgumentException("Reference site count exceeds occurrence count");
        }
    }
}
