package com.github.tth05.jindex;

import org.junit.jupiter.api.Test;

import java.util.EnumSet;
import java.util.List;
import java.util.Objects;

import static org.junit.jupiter.api.Assertions.*;

final class ApiContractTest {
    private static final EnumSet<SymbolKind> KINDS = EnumSet.of(SymbolKind.FIELD, SymbolKind.METHOD);

    @Test
    void symbolLimitsReportTruncationIncludingZeroAndExactBoundary() throws Exception {
        try (ClassIndex index = fixtureIndex()) {
            for (var mode : SearchOptions.SearchMode.values()) {
                for (int limit = 0; limit <= 3; limit++) {
                    SymbolSearchPage page = index.findSymbols("match", SearchOptions.with(mode,
                            SearchOptions.MatchMode.MATCH_CASE, limit), KINDS);
                    assertEquals(Math.min(limit, 2), page.results().length);
                    assertEquals(limit < 2, page.truncated());
                    assertFalse(index.findSymbols("missing", SearchOptions.with(mode,
                            SearchOptions.MatchMode.MATCH_CASE, limit), KINDS).truncated());
                }
            }
            assertFalse(index.findSymbols("match", SearchOptions.defaultOptions(), EnumSet.noneOf(SymbolKind.class)).truncated());
            SymbolSearchPage page = index.findSymbols("match", SearchOptions.defaultOptions(), KINDS);
            page.results()[0] = null;
            assertNotNull(page.results()[0]);
        }
    }

    @Test
    void emptySourceSelectionIsDistinctFromTheUnfilteredOverload() throws Exception {
        try (ClassIndex index = fixtureIndex()) {
            assertEquals(2, index.findSymbols("match", SearchOptions.defaultOptions(), KINDS).results().length);
            assertEquals(2, index.findSymbols("match", SearchOptions.defaultOptions(), KINDS, 42, 42).results().length);
            assertEquals(0, index.findSymbols("match", SearchOptions.defaultOptions(), KINDS, new int[0]).results().length);
            assertFalse(index.findSymbols("match", SearchOptions.defaultOptions(), KINDS, new int[0]).truncated());
            assertEquals(0, index.findClasses("ApiContractTest", SearchOptions.defaultOptions(), new int[0]).results().length);
            assertEquals(0, index.findClassesByBinaryName("Fixture", SearchOptions.defaultOptions(), new int[0]).results().length);
        }
    }

    @Test
    void closeIsIdempotentAndRejectsSubsequentQueries() throws Exception {
        ClassIndex index = fixtureIndex();
        index.close();
        assertTrue(index.isDestroyed());
        assertDoesNotThrow(index::close);
        assertThrows(IllegalStateException.class, index::getStatistics);
    }

    private static ClassIndex fixtureIndex() throws Exception {
        try (var bytes = Objects.requireNonNull(Fixture.class.getResourceAsStream("/" + Fixture.class.getName().replace('.', '/') + ".class"))) {
            return ClassIndex.fromSources(List.of(IndexSource.classFile(42, bytes.readAllBytes())));
        }
    }

    static final class Fixture {
        int matchField;
        void matchMethod() {}
    }
}
