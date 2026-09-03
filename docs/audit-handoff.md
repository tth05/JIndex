# JIndex audit implementation handoff

Work is in progress on `codex/reference-index-1.1`, directly in `C:/Users/Admin/IdeaProjects/JIndex`. The starting commit is `5833362032d49af75b483da1e247e643031165b7`. The user explicitly requested the existing checkout, not a separate worktree.

The source plan is `C:/Users/Admin/.claude/plans/audit-this-repository-for-tingly-brooks.md`. The user wants implementation and hard testing here, then review and improvements by the agent that wrote that plan. Do not treat the original plan's claims as verified evidence.

## Completed correctness work

- Shared, bounded ASCII search with the documented first-match-character case rule.
- Constant-pool views store length, including 255-byte strings. Empty detection now checks length rather than comparing length with a pool offset. Oversized Unicode error formatting no longer panics.
- Hierarchy queries reset traversal state and detect cycles. Base-method traversal is iterative and deterministic.
- Signature parsing validates ASCII before creating ASCII references, rejects malformed generic prefixes, and reports a missing class superclass as an error. Unsupported Unicode signatures are rejected explicitly rather than silently losing generic metadata.
- Exception arrays contain only resolved classes, including resolved generic bounds.
- Empty package comparison and corrupted signature tags fail safely.
- Exact class lookup rejects null names at the public Java boundary.
- Generic erased-type comparison is symmetric and excludes primitive/type-variable matches.
- JNI pointer borrows are tied to the Java object's borrow rather than fabricated as static. Null pointer guards and safety contracts are documented at the conversion boundary. Native destruction uses the JNI panic boundary and accepts a null pointer as a no-op.
- Java cleanup atomically claims its weak registry entry and has one native free call site, including failed Cleaner registration.

## Evidence so far

`cargo test --all-targets --locked audit_ -- --test-threads=1` failed on all five initial native regressions before their fixes. A small external Java fixture reproduced the prefix panic, an unrelated hierarchy implementation, and a null exception result. New Java regressions also failed on hierarchy, null-name validation and exception arrays before the corresponding fixes.

`./gradlew.bat build benchmarkRuntimeCorpus -PbenchmarkManifest=C:/Users/Admin/.codex/worktrees/jindex-audit/evidence/runtime-sources.txt -PbenchmarkOutput=C:/Users/Admin/.codex/worktrees/jindex-audit/evidence/correctness-baseline.json --console=plain` passed after the first correctness pass: 34 Java tests, 36 native tests. Javadoc emits pre-existing missing-comment warnings. The Cleaner registry assertion was subsequently strengthened to inspect actual registry removal. The full `./gradlew.bat check --console=plain` then passed, including that assertion and the new all-target Clippy warnings-as-errors gate.

The untouched baseline could not finish this corpus benchmark because prefix search panicked. Performance comparisons therefore start after correctness fixes and before performance changes.

The current runtime inventory supplies 611 archive sources, not the old audit's 322 mods or the old benchmark's 563 runtime archives. Manifest SHA-256: `30d5c571114e3cfa4c78c7ee4f59816d3fab5bfd048e9d9c29dfd573071a069f`. Java is Temurin 21.0.12. Initial corrected measurements: build 10.589 seconds, save 1.538 seconds, first load 699 ms, warm load median 621 ms, persisted bytes 85,620,087. Selected classes 178,106, fields 605,299, methods 1,299,629, reference sites 14,879,170, reference occurrences 39,479,848, literals 609,239, literal occurrences 2,047,438. Raw results and manifest are in the evidence directory above, which contains artifacts only and is not a Git worktree.

## Remaining work

1. Correctness and the Clippy gate are ready for the first checkpoint. Keep performance changes in later checkpoints.
2. Clean up the agreed dead code and improve native DLL extraction. Preserve intentionally boxed vector layout with a measured rationale rather than claiming boxed slices are the same size.
3. Implement and measure package-name caching, matching sort/lookup changes with a snapshot version bump, reference target deduplication and safe parallel resolution, and streaming persistence. Compare exact resolved references against an uncached test oracle, not just aggregate counts.
4. Complete the added native lifetime tests and API contract tests. The API review includes missing symbol truncation metadata, bounded results without cursors, empty source selection, the destroy alias, and native/Java development version alignment. These additions were requested for testing; decisions that change public API need a stated rationale and coordinated consumer verification.
5. Run final quality gates, corpus comparisons, and relevant Companion integration verification. Publish to Maven Local only for coordinated local consumer verification. No deployment was requested.
6. Replace this progress document with final commits, measurements, exact reproduction commands, and review questions for the original planning agent.

## Plan corrections to retain

- Forward package ordering changes a persisted lookup invariant, even if cached names are derived. Bump the snapshot version and reject old snapshots.
- Package-name allocations happen only on equal class names in `then_with`, not every sort comparison. Measure them.
- The old 10–12-second benchmark measures the whole build, not reference resolution alone.
- On this 64-bit toolchain, `Option<Box<Vec<T>>>` is 8 bytes and `Option<Box<[T]>>` is 16. Measure total memory before changing this layout.
- The existing reference lookup contains `&IndexedClass`, which is not Sync. Parallel workers need a separate map of names to IDs or another safe immutable representation.
- Companion already filters null exception entries. The defect was in the API result, not an observed Companion null crash.
- The current runtime inventory is JSON. The evidence manifest was derived from its ordered archive URIs after verifying that each archive exists; the old benchmark still consumes its versioned line format.
