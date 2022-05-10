package com.github.tth05.jindex;

public class SearchOptions {

    private int limit;
    private SearchMode searchMode;
    private MatchMode matchMode;

    private SearchOptions(SearchMode searchMode, MatchMode matchMode, int limit) {
        this.limit = limit;
        this.searchMode = searchMode;
        this.matchMode = matchMode;
    }

    public static SearchOptions defaultOptions() {
        return new SearchOptions(SearchMode.PREFIX, MatchMode.IGNORE_CASE, Integer.MAX_VALUE);
    }

    public static SearchOptions with(SearchMode searchMode, MatchMode matchMode, int limit) {
        return new SearchOptions(searchMode, matchMode, limit);
    }

    public static SearchOptions defaultWith(SearchMode searchMode, MatchMode matchMode) {
        SearchOptions options = defaultOptions();
        options.searchMode = searchMode;
        options.matchMode = matchMode;
        return options;
    }

    public static SearchOptions defaultWith(SearchMode searchMode) {
        SearchOptions options = defaultOptions();
        options.searchMode = searchMode;
        return options;
    }

    public static SearchOptions defaultWith(MatchMode matchMode) {
        SearchOptions options = defaultOptions();
        options.matchMode = matchMode;
        return options;
    }

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
