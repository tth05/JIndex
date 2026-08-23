# JIndex 1.1 declaration, reference, and literal index

Status: proposed format, accepted scope

## Objective

Build one immutable index snapshot that answers class, field, method, Find Usages, source-filter, and Java string-literal queries without reopening the indexed class files. Keep build memory, resident memory, persisted size, load time, query latency, and JNI allocation measurable and bounded.

## Current baseline

JIndex currently parses class declarations with bytecode parsing disabled. It retains names, access flags, signatures, enclosing relationships, packages, fields, methods, and type hierarchy. Names are interned into one length-prefixed ASCII byte pool and declarations refer to them by integer offset. The persisted snapshot uses explicit `speedy` serialization inside one deflated ZIP entry.

Companion currently reopens every runtime class for each Find Usages request. Its ASM scanner is the behavioral authority for class, field, method, descriptor, annotation, handle, `invokedynamic`, inheritance, multi-release archive, progress, and cancellation semantics.

## Required queries

The 1.1 design must support these operations without scanning original class sources:

1. Find classes by name with prefix and contains matching.
2. Find fields and methods by name, returning their declaring class and descriptor.
3. Resolve exact declarations by class, name, and descriptor.
4. Find all indexed reference sites for one exact class, field, method, or constructor symbol.
5. Find Java string literals by text and return every occurrence site.
6. Restrict symbol and reference results to a caller-selected set of class-source IDs.

Queries return deterministic ordering and explicit truncation metadata. JNI returns batches rather than one call or wrapper allocation per result.

## Semantic model

### Symbol identity

Every selected declaration receives an index-local ID. A field or method symbol includes its declaring class, name, and JVM descriptor. Constructors use the JVM name `<init>` and remain method symbols.

The persisted representation may use a tagged integer ID if measurements show that it reduces reference storage without slowing queries. Public Java APIs expose typed results and do not expose bit layouts.

### Reference identity

A reference records:

- exact target symbol;
- containing class, field, or method declaration;
- reference kind;
- optional instruction or attribute position only if it materially improves deterministic ordering or future navigation.

Repeated uses of the same target inside the same reference site are counted, but the public API can request either distinct sites or individual occurrences. Find Usages defaults to distinct sites because Companion navigates to decompiled members rather than bytecode offsets.

### Reference coverage

The extractor must cover:

- class inheritance and implemented interfaces;
- field and method descriptors;
- generic signatures and declared exceptions;
- runtime-visible and runtime-invisible annotations, type annotations, defaults, enums, and nested annotation values;
- field and method instructions;
- allocations, casts, type tests, arrays, and class literals;
- method handles, method types, constant dynamics, bootstrap arguments, and `invokedynamic`;
- enclosing classes, nestmates, permitted subclasses, records, and inner classes;
- multi-release archive selection according to the target Java release.

Exact bytecode owners remain available. Hierarchy-aware queries expand inherited field or method ownership using the declaration graph rather than rewriting stored facts.

### String literals

The literal index contains semantic Java string values, not every class-file UTF-8 entry. Initial coverage includes:

- `ldc` strings;
- `ConstantValue` strings;
- annotation and annotation-default strings;
- string bootstrap constants that represent program values.

Compiler protocol strings such as `StringConcatFactory` recipes are classified separately and are not shown as source literals unless they contain an independently represented constant value.

The literal pool cannot reuse the ASCII name pool. It must preserve all Java `String` values, including embedded nulls and unpaired UTF-16 surrogates. The exact persisted encoding is a benchmarked format decision. WTF-8 and UTF-16 code-unit storage are the initial candidates.

## Source identity and provenance

The caller supplies class sources in deterministic precedence order. Every selected class stores the ID of the source that supplied it. Duplicate-class resolution and the stored source ID must agree.

JIndex treats source IDs as opaque. TotalDebug will publish a side manifest that maps each source ID to mod ID, display name, physical archive, source kind, and JDK module where applicable. Companion joins query results to this manifest for display and filtering.

## Storage requirements

The runtime representation must avoid one heap allocation per reference, literal occurrence, or search key. New high-cardinality data uses contiguous arrays or arenas.

The candidate reference representation is a target-sorted adjacency index:

```text
target_offsets[target_id] .. target_offsets[target_id + 1]
    -> packed_reference_sites[]
```

Candidate compression techniques include:

- source, class, member, and literal integer IDs;
- kind tags packed into unused high bits when count limits are proven;
- sorted postings with delta or variable-length encoding on disk;
- one global name pool and one Unicode literal pool;
- flattened class-to-field and class-to-method ranges;
- source-ID bitsets for filtered result sets.

No bit width, compression scheme, mmap strategy, or text-search accelerator is accepted until it is measured against the production corpus. The code must assert every chosen count limit and reject overflow exactly.

## Persisted format

The 1.1 snapshot will have an explicit magic value, format version, section directory, element counts, and integrity validation. Unknown versions fail with a precise error. Consumers rebuild generated snapshots instead of attempting fallback decoding.

Candidate sections are:

1. name pool;
2. Unicode literal pool;
3. class sources;
4. packages;
5. classes;
6. fields;
7. methods;
8. type and generic signatures;
9. name-search accelerators;
10. target-sorted reference postings;
11. literal-search accelerator and occurrence postings.

The baseline benchmark decides whether sections remain in the current deflated container, use independently compressed blocks, or become directly memory-mappable. Persisted size cannot be optimized at the cost of unbounded load-time allocation.

## Text search strategy

All literal values are deduplicated before indexing occurrences. The benchmark compares:

- scanning the deduplicated literal pool in native code;
- a trigram-to-literal postings index with final candidate verification;
- any simpler prefix index needed by observed UI queries.

The accepted strategy must define behavior for queries shorter than three code points, case sensitivity, Unicode matching, result limits, and cancellation. Display truncation never truncates indexed or matched data.

## Correctness strategy

1. Preserve every existing JIndex declaration, hierarchy, persistence, and lifecycle test.
2. Port the Companion reference fixtures into a language-neutral expected-result corpus.
3. Run the current ASM engine and JIndex against identical class files and compare exact normalized results.
4. Add focused fixtures for Unicode literals, embedded nulls, unpaired surrogates, annotations, constant dynamics, string concatenation, lambdas, bridge methods, records, sealed classes, and duplicate source precedence.
5. Fuzz class parsing, persisted-section validation, and query bounds.
6. Verify save-load-query equality and deterministic byte output.

The Java scanner is a test oracle, never a production fallback.

## Benchmark stage

The first executable milestone is a repeatable benchmark over the current ATM10 Skyblock runtime-source manifest. It records:

- source count, selected classes, declarations, and class-file bytes;
- current JIndex build time, peak resident memory, persisted size, cold load, and warm load;
- current class-query latency and allocation;
- current Companion full reference-scan latency for representative class, field, and method targets;
- class-file string counts, unique semantic literals, Unicode distribution, and occurrence counts;
- projected reference counts by kind.

Every run records JDK, CPU, commit, source-manifest signature, warmup policy, sample count, and raw measurements. Benchmarks write machine-readable results and a short human summary outside the committed source tree.

After the first baseline, implementation prototypes must report deltas against it. Format choices require measurements for at least build time, file size, load time, resident memory, and p50/p95 query latency.

## Implementation slices

### Slice A: baseline and format proof

- Add a standalone benchmark entry point that consumes a runtime-source manifest.
- Record the current metadata index and on-demand reference scanner baselines.
- Add struct-size and serialized-size accounting.
- Prototype the section header and reject malformed or unknown versions.

### Slice B: flattened declarations and sources

- Add deterministic source IDs and source-aware duplicate selection.
- Flatten declaration storage and introduce typed global symbol IDs.
- Implement batched class, field, and method search APIs.
- Preserve declaration and hierarchy behavior through differential tests.

### Slice C: semantic reference graph

- Parse method code and semantic attributes once during the parallel build.
- Resolve targets after every declaration has an ID.
- Build compact target-sorted postings.
- Add batched exact-reference queries and hierarchy expansion.
- Match the Companion oracle on every reference fixture.

### Slice D: literal index

- Extract and preserve semantic Java string values.
- Add the chosen literal encoding and search strategy.
- Return literal occurrences with their reference sites and source IDs.
- Cover Unicode and compiler-generated cases.

### Slice E: consumer integration

- Publish the JIndex artifact and format version through Maven Local.
- Extend TotalDebug's runtime-source manifest with provenance keyed by source ID.
- Replace Companion's class-only popup with `All`, `Classes`, and `Symbols` results plus source filters.
- Route Find Usages through JIndex and remove the production ASM scan after parity.
- Keep resource-file text search outside JIndex.

## Acceptance

The upgrade is complete only when all four repositories build from clean checkouts, the production modpack snapshot rebuilds deterministically, every current reference result remains available, symbol and literal queries meet measured interactive latency, source filters are exact, and Companion no longer reparses the runtime classpath for Find Usages.
