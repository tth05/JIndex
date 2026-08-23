package com.github.tth05.jindex;

public record IndexStatistics(
        long classCount,
        long fieldCount,
        long methodCount,
        long referenceSiteCount,
        long literalCount,
        long literalOccurrenceCount
) {
}
