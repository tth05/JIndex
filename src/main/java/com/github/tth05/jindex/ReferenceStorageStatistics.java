package com.github.tth05.jindex;

/** Distribution data used to evaluate compact reference-posting layouts against real indexes. */
record ReferenceStorageStatistics(
        long siteCount,
        long occurrenceCount,
        long singleOccurrenceSiteCount,
        long countOver255SiteCount,
        long countOver65535SiteCount,
        long maximumOccurrenceCount
) {
}
