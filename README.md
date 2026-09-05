# JIndex

JIndex builds an immutable index of JVM class sources. Rust owns the index and Java 21 accesses it through JNI. The packaged native library currently targets Windows x64.

## Indexed data and queries

The index exposes packages, classes, fields and methods, including JVM descriptors, generic signatures, source IDs, nesting and declaration modifiers. Queries cover class names and binary names, packages, field/method symbols, semantic references, string literals and hierarchy relationships. Search pages report truncation; symbol and reference queries can select caller-assigned source IDs.

Class sources can be archives or loose class bytes. A source ID identifies input within one index; the caller owns its meaning. Snapshots can be saved and reopened. Objects and IDs obtained from an index expire when that index closes. Use try-with-resources; `close()` is idempotent and releases native memory.

Name lookup uses the native ASCII representation; this is not a promise of unrestricted Unicode identifier support. Generic signatures remain available as strings. String-literal queries represent semantic literal occurrences rather than every UTF-8 constant-pool entry. See [the domain model](CONTEXT.md) and [the implementation plan](docs/index-1.1-plan.md) for the precise scope and remaining acceptance work.

## Build and dependency

Install a full JDK 21 and the Rust toolchain pinned in [rust-toolchain.toml](jindex-rs/rust-toolchain.toml), including Clippy and rustfmt. On Windows:

```powershell
.\gradlew.bat clean build publishToMavenLocal --warning-mode fail
```

The wrapper builds the locked native dependency graph, stages the DLL under `build/`, packages the Java API, and runs Java tests, native tests, formatting and Clippy. Push/PR builds run the same checks on Windows.

The current source defaults to `1.1.0-SNAPSHOT` for local development. Release builds set `-PjindexVersion=<version>`. Publish the selected immutable release before using it in public consumers:

```groovy
repositories {
    maven { url = uri('https://packagecloud.io/tth05/repo/maven2') }
}
dependencies {
    implementation 'com.github.tth05:jindex:RELEASE_VERSION'
}
```

Replace `RELEASE_VERSION` with a published release. Local coordinated consumers use Maven Local explicitly until that publication exists.

## Performance and acceptance

Use [the benchmark record](docs/index-1.1-benchmark.md) for measured results, corpus hashes, source revisions and commands. Its stages measure different index contents and should not be compared as one universal speed or memory claim.

The runtime corpus includes 175,265 class inputs from ATM10 To the Sky and a Java 21 runtime. The benchmark records build/load timing, query latency and process working set. A full corpus run is separate from ordinary CI because its input archives are external. Before a stable release, repeat it with the recorded manifest and candidate bytes. Independent ASM extraction parity and the remaining malformed-input acceptance cases are still open in the release audit; the existing Rust resolver comparison is not an independent extractor.
