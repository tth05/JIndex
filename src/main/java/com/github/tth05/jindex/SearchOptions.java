package com.github.tth05.jindex;

import java.util.Objects;

/**
 * Immutable options shared by name-based index searches.
 *
 * @param searchMode where a match may begin
 * @param matchMode how letter case is compared
 * @param limit maximum number of results to return
 */
public record SearchOptions(SearchMode searchMode, MatchMode matchMode, int limit) {

    /**
     * Creates validated search options.
     */
    public SearchOptions {
        Objects.requireNonNull(searchMode, "searchMode");
        Objects.requireNonNull(matchMode, "matchMode");
        if (limit < 0) {
            throw new IllegalArgumentException("limit must not be negative");
        }
    }

    /**
     * Returns the default case-insensitive prefix options without a practical result limit.
     *
     * @return default search options
     */
    public static SearchOptions defaultOptions() {
        return new SearchOptions(SearchMode.PREFIX, MatchMode.IGNORE_CASE, Integer.MAX_VALUE);
    }

    /**
     * Creates search options with an explicit result limit.
     *
     * @param searchMode where a match may begin
     * @param matchMode how letter case is compared
     * @param limit maximum number of results to return
     * @return validated search options
     */
    public static SearchOptions with(SearchMode searchMode, MatchMode matchMode, int limit) {
        return new SearchOptions(searchMode, matchMode, limit);
    }

    /** Describes where a query may match a candidate name. */
    public enum SearchMode {
        /**
         * The match has to occur at the start of the string.
         */
        PREFIX,
        /**
         * The match can occur anywhere in the string.
         */
        CONTAINS
    }

    /** Describes how letter case participates in matching. */
    public enum MatchMode {
        /**
         * The match is case-insensitive.
         */
        IGNORE_CASE,
        /**
         * The match is case-sensitive.
         */
        MATCH_CASE,
        /**
         * The match is case-sensitive, but only for the first character of where the match occurs.
         */
        MATCH_CASE_FIRST_CHAR_ONLY,
    }
}
