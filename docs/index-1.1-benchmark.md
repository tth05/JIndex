# JIndex 1.1 benchmark

This benchmark fixes the comparison point for the semantic-reference work. It measures the mixed-source path introduced by `8af9d14`, not the older synthetic-JDK-JAR path.

## Corpus

The input is the runtime-source manifest produced by TotalDebug for the ATM10 To the Sky instance, plus the classes exposed by the same Java 21 `jrt:/` image used to run Minecraft.

| Input | Value |
|---|---:|
| Runtime archives | 563 |
| Runtime archive bytes | 690,695,850 |
| Archive class inputs | 147,428 |
| JDK class inputs | 27,837 |
| Total class inputs | 175,265 |
| Uncompressed class bytes | 781,514,895 |

The manifest SHA-256 is `e7284a49c5d6cf8001e46bf42303f72e06ae28e182ef27b6a4add60f35393c47`. The benchmark fails if the manifest does not use TotalDebug's versioned file-URI format, if an input is missing, or if representative Minecraft and JDK classes are absent from the resulting index.

## Baseline

The representative run used Temurin 21.0.12 on a 12-processor Windows host. It launched the benchmark in a fresh JVM without a Gradle daemon in the measured process.

| Measurement | Baseline |
|---|---:|
| Read JDK loose classes | 445.4 ms |
| Java-observed native build call | 2,786.1 ms |
| Native class reading | 1,304 ms |
| Native metadata construction | 1,010 ms |
| Save compressed index | 2,288.9 ms |
| Persisted index size | 15,562,682 bytes |
| First load | 320.6 ms |
| Warm load p50 | 263.9 ms |
| Warm load p95 | 338.9 ms |
| Peak process working set | 1,724,370,944 bytes |

The peak includes loose JDK byte arrays, native build state, the completed native index, query samples, and repeated load verification. It is a process-level ceiling for this benchmark, not a retained-heap measurement.

Current query latency:

| Query | p50 | p95 |
|---|---:|---:|
| Exact class | 2.5 us | 3.0 us |
| Case-insensitive class prefix | 77.4 us | 136.0 us |
| Case-insensitive class contains | 236.5 us | 305.0 us |

## Flattened declaration slice

The first 1.1 implementation slice stores source IDs, exact JVM descriptors, typed field and method IDs, and name-sorted member lookup tables. The run below uses the same manifest and fresh-JVM process shape as the baseline.

| Measurement | Metadata baseline | Declaration slice |
|---|---:|---:|
| Selected classes | 170,213 | 170,213 |
| Fields | not recorded | 584,761 |
| Methods | not recorded | 1,244,579 |
| Java-observed native build call | 2,786.1 ms | 6,584.5 ms |
| Native class reading | 1,304 ms | 1,766 ms |
| Native index construction | 1,010 ms | 4,147 ms |
| Save compressed index | 2,288.9 ms | 4,156.9 ms |
| Persisted index size | 15,562,682 bytes | 27,562,209 bytes |
| First load | 320.6 ms | 470.4 ms |
| Warm load p50 | 263.9 ms | 405.6 ms |
| Warm load p95 | 338.9 ms | 516.5 ms |
| Peak process working set | 1,724,370,944 bytes | 1,930,379,264 bytes |

Member query latency:

| Query | p50 | p95 |
|---|---:|---:|
| Exact method-name prefix | 25.5 us | 57.2 us |
| Case-insensitive field and method prefix | 329.5 us | 584.1 us |
| Case-insensitive field and method contains | 67.4 ms | 77.8 ms |

Prefix lookup binary-searches name-sorted packed member IDs and reads at most the requested result range. Contains lookup walks each class's contiguous field and method arrays. The contains result includes both kinds. An earlier 31.6 ms prototype omitted methods whenever fields filled the result limit, so it is not a valid comparison point.

The declaration slice increases persisted size by 12.0 MB and peak working set by 206.0 MB. Most of the retained increase comes from 1,829,340 packed member IDs plus exact descriptor storage. The sort and larger compressed snapshot account for most of the cold-build and save cost. These costs stay visible while the reference representation is developed. They are not treated as free or hidden inside the original baseline.

The full Java/native suite passed 11 tests after this run. It covers mixed-source precedence, source identity, cross-kind result limits, prefix and contains ordering, exact descriptors, persistence, lifecycle safety, and precise snapshot-version failures.

## Class-file parser prerequisite

Reference extraction needs code, bootstrap methods, annotations, records, sealed-class metadata, and the newer constant-pool forms in one parse. The pinned `cafebabe` fork deliberately disabled most attribute parsing, so JIndex now uses `jvmti-bindings` 3.0.2 and removes the old parser.

The migration was checked against the same production corpus. It selects exactly 170,213 classes, 584,761 fields, and 1,244,579 methods, matching the declaration slice. The persisted file differs by 11 bytes because the parser representations are not byte-identical before compression. A clean 13-test suite covers records, sealed classes, string-concat bootstrap metadata, source precedence, persistence, and lifecycle behavior.

One production class contains the method `generateGrötzschGraph`. JVM member names may be Unicode, but JIndex's compact declaration-name pool and search API remain ASCII-only. The builder therefore retains that class and its 77 supported methods while excluding the one unsupported declaration, which is the previous format's behavior. Other parse failures now identify the archive and entry and fail the build instead of silently omitting a class.

## Runtime archive view

Archive indexing now applies the Java multi-release JAR rules before parsing classes. A manifest opt-in selects the highest `META-INF/versions/<N>` entry no newer than the configured target Java release; ordinary archives ignore versioned entries. `IndexBuildOptions` makes that release explicit and defaults to the running JVM.

This correctness change establishes the new production totals at 170,212 classes, 584,761 fields, and 1,244,596 methods. The previous reader parsed every versioned entry and resolved same-source duplicates after parallel parsing, so its 170,213/584,761/1,244,579 totals did not describe one real runtime view. The unit suite checks Java 17 and Java 21 views of a multi-release fixture and confirms that a non-multi-release archive exposes only its base class.

## Semantic references and string literals

The semantic slice parses code and attributes in the same pass as declarations. It resolves
member owners after declaration IDs exist, then stores target-sorted reference postings as one
eight-byte packed site and occurrence count. Construction uses a temporary twelve-byte resolved
posting so raw parser data can be released before the final array is sorted and compacted.

The production runtime view contains 14,308,440 distinct target/site postings. The first global-sort
prototype built in 16.24 seconds and reached a 3.21 GB process working set. The accepted staged
builder reduced the measured build call to 11.65 seconds while producing the same postings. A later
same-machine run measured 10.01 seconds; cold build timing remains secondary to correctness until
the graph and consumer integration are complete. The
whole-process working-set ceiling remained sensitive to the benchmark's repeated 43,335-object
class-reference query; it is therefore retained as a ceiling rather than reported as native retained
index memory.

String literals use exact UTF-16 code units. The extractor includes `ldc`, `ConstantValue`,
annotation/default values, and semantic bootstrap string constants, while excluding the recipe
argument of `StringConcatFactory.makeConcatWithConstants`. The corpus contains 586,326 unique
values and 1,951,942 occurrences. Lone surrogates and embedded nulls remain representable.

| Measurement | Declaration slice | References + literals |
|---|---:|---:|
| Selected classes | 170,213 | 170,212 |
| Fields | 584,761 | 584,761 |
| Methods | 1,244,579 | 1,244,596 |
| Distinct reference sites | not indexed | 14,308,440 |
| Unique literals | not indexed | 586,326 |
| Literal occurrences | not indexed | 1,951,942 |
| Java-observed native build call | 6,584.5 ms | 11,649.6 ms |
| Save compressed index | 4,156.9 ms | 11,907.2 ms |
| Persisted index size | 27,562,209 bytes | 76,304,860 bytes |
| First load | 470.4 ms | 855.6 ms |
| Warm load p50 | 405.6 ms | 812.6 ms |
| Warm load p95 | 516.5 ms | 869.8 ms |

Reference queries are bounded before JNI object construction. The table reports the total stored
site count and the latency to return the first 200 deterministic sites, including Java result
construction:

| Query | Stored sites | Returned | p50 | p95 |
|---|---:|---:|---:|---:|
| Class `Block` | 43,335 | 200 | 0.239 ms | 0.401 ms |
| Field `Blocks.AIR` | 966 | 200 | 0.301 ms | 0.376 ms |
| Method `Block.defaultBlockState` | 4,509 | 200 | 0.291 ms | 0.514 ms |
| Exact literal `minecraft` | 349 | 200 | 0.288 ms | 0.454 ms |
| Literal contains `block`, limit 200 | n/a | 200 | 5.06 ms | 6.63 ms |

Source filters are applied against the containing class's opaque source ID in the same native loop,
before the page limit and before JNI allocation. Truncation therefore describes the selected source
set, not the unfiltered posting list.

The literal pool is sorted by UTF-16 code units, giving binary exact lookup and deterministic prefix
order. Contains search scans the deduplicated pool. A trigram index is rejected for this corpus:
the measured scan is already interactive, while n-grams would add another large postings structure,
increase build memory, and complicate the snapshot.

## Comparison rules

Every format experiment must use the same manifest, Java runtime, and benchmark process shape. Report at least:

- semantic edge and literal counts;
- build and save time;
- persisted bytes;
- peak working set;
- first and warm load time;
- class, symbol, reference, and literal query p50/p95;
- correctness against the existing Companion bytecode scanner on a fixed target corpus.

Do not accept a size or speed improvement that drops a reference kind, changes duplicate precedence, loses non-ASCII literal data, or silently omits an input class.
