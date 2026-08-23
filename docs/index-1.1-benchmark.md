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
