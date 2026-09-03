# JIndex audit handoff

Implementation and regression testing are ready for the original planning agent's review. Work stayed in the existing checkouts. No deployment or public release was performed.

Source plan: `C:/Users/Admin/.claude/plans/audit-this-repository-for-tingly-brooks.md`.

## Review these changes

JIndex starts at `5833362032d49af75b483da1e247e643031165b7` on `codex/reference-index-1.1`.

| Commit | Review focus |
|---|---|
| `c2782e9` | Correctness regressions, JNI lifetime guards, Clippy gate |
| `012b5df` | Cached package names, matching forward sort/lookup, streaming snapshot version 5 |
| `3b03e90` | Global reference-target deduplication, parallel resolution, independent per-edge test oracle |
| `8e72733` | Reverse hierarchy adjacency, cycle tests, dead-code cleanup, checked pool access |
| `92cc239` | Symbol truncation metadata, native DLL cache, API tests, development version alignment |
| `ec402ab` | Byte-based search comparisons while keeping typed string getters checked |

Companion commit `de5d042` follows `20a95ee` in `C:/Users/Admin/IdeaProjects/TotalDebugCompanion`. It updates the two symbol-search callers and adds the failing-then-passing MCP truncation regression. Other agents' runtime recovery changes were preserved.

## Evidence

- The first native regression run failed all five new cases before their fixes. Java reproductions exposed the prefix panic, unrelated hierarchy result and null exception entry.
- The package-order test failed before the sort change and passes both before and after persistence.
- The MCP test generates 101 matching fields. Previously it returned 100 results without reporting truncation. It now reports truncation.
- JIndex's full build passed with 41 Java tests and 43 native tests. The corpus-only native test is intentionally ignored by the ordinary gate and was run separately in release mode.
- The exact corpus comparison used 611 archives plus all 27,837 classes from the benchmark's JDK runtime image. Every resolved target, source-site identity, relation mask and occurrence count matched the original per-edge implementation. The final graph has 178,106 classes and 14,879,170 reference sites.
- The sample corpus serializes identically with one and four Rayon worker threads.
- Hierarchy queries match an independent forward walk over 256 generated graphs, including cycles and diamonds. Malformed Java hierarchy fixtures also terminate.
- Lifetime tests cover close/read races over 100 rounds, concurrent closers, children retaining owners, collected owners triggering Cleaner removal, null JNI pointers and close waiting for an in-flight child call.
- Persistence tests reject old versions, malformed tags, truncated payloads, trailing payload bytes and wrong ZIP checksums.
- Native-library tests cover concurrent extraction, tampered-cache rejection, and two JVMs each loading JIndex through three class loaders. Each class loader has isolated native field IDs; cache slots are reused across JVMs.
- Companion's clean full rerun passed 399 tests after Maven Local publication. An earlier run overlapped publication and logged exception-class loading errors; the non-overlapping rerun had none.

Timing, corpus hashes and measurement limits are in the [audit benchmark section](index-1.1-benchmark.md#audit-pass-2026-09-03). Raw JSON and the exact inputs are in `C:/Users/Admin/.codex/worktrees/jindex-audit/evidence`. That directory contains artifacts, not a Git worktree.

## Reproduce

Use Java 21 at `C:/Users/Admin/.jdks/temurin-21.0.12`. From JIndex:

```powershell
$env:JAVA_HOME='C:\Users\Admin\.jdks\temurin-21.0.12'
.\gradlew.bat build publishToMavenLocal --console=plain
.\gradlew.bat benchmarkInitialIndex --args=mixed-jdk --console=plain

$env:JINDEX_BENCHMARK_COMMIT=git rev-parse HEAD
.\gradlew.bat benchmarkRuntimeCorpus '-PbenchmarkManifest=C:/Users/Admin/.codex/worktrees/jindex-audit/evidence/runtime-sources.txt' '-PbenchmarkOutput=build/audit-runtime.json' --console=plain

$env:JINDEX_AUDIT_MANIFEST='C:\Users\Admin\.codex\worktrees\jindex-audit\evidence\runtime-sources.txt'
$env:JINDEX_AUDIT_JDK_ARCHIVE='C:\Users\Admin\.codex\worktrees\jindex-audit\evidence\runtime-jdk.zip'
Push-Location jindex-rs
cargo test --release --all-targets --locked audit_runtime_corpus_reference_equivalence -- --ignored --nocapture
Pop-Location
```

To regenerate the JDK ZIP, set `JINDEX_AUDIT_JDK_ARCHIVE` before running the runtime benchmark. JMOD files are not equivalent: this JDK image contains 18 additional classes. The test driver also preserves the Java mixed-source overload's JDK-first duplicate precedence.

After publication finishes, run `.\gradlew.bat test --rerun-tasks --console=plain` in Companion. Do not republish its dependency while the test JVM is running.

## Decisions to verify before release

1. Snapshot version 5 is intentional. Forward package ordering changes the persisted binary-search invariant even though cached names themselves are derived. Old snapshots are rejected, not migrated.
2. Unsupported non-ASCII signatures fail explicitly. The original descriptor-fallback suggestion would silently discard generic metadata; it was not implemented.
3. `findSymbols` now returns `SymbolSearchPage`. This fixes a real Companion bug: requesting 100 results and then testing array length greater than 100 can never detect truncation.
4. All pages remain bounded results without cursors. Explicit empty source selection returns nothing; the overload without a source filter searches all sources. `destroy()` remains a tested close alias. Decide whether to remove it before freezing the API.
5. Java and Cargo both say `1.1.0-SNAPSHOT`. This aligns development metadata only. The final public version number and API freeze remain owner decisions.
6. JNI safety still depends on the documented Java lifecycle lock and valid native pointers. Borrow lifetimes no longer pretend to be static, but this is not a Rust-owned handle registry or proof against forged pointers. No sanitizer, Miri or live Minecraft deployment was run.
7. Optional immutable `IndexedClass` construction and bounded top-k contains search were not implemented. The boxed-vector layout is deliberately retained: an optional boxed vector is one pointer, while an optional boxed slice is two.
8. Hierarchy acceleration preserves the existing signature-derived parent edges. Review the treatment of implicit `java/lang/Object` parents separately; the graph optimization is not a redesign of erased-type or Object-root semantics.

For a focused review, start with the native lifetime boundary, target-key normalization/oracle comparison, snapshot ordering, and the symbol API change. The corpus oracle proves equivalence to the existing resolver, not independent correctness of every JVM-resolution rule.
