# JIndex

JIndex builds an immutable, searchable index of JVM class sources. Rust owns the index and Java 21 accesses it through JNI. The packaged native library targets **Windows x64**.

## Data and queries

The index exposes packages, classes, fields and methods, including JVM descriptors, generic signatures, source IDs, nesting and declaration modifiers. Queries cover class and member names, semantic references, string literals and type hierarchies. Search pages report truncation; symbol and reference queries can filter by source ID.

Sources can be archives or loose class bytes. Their order determines duplicate-class precedence. Multi-release JAR selection follows the target Java release. Source IDs identify inputs within one index; callers supply their application-specific meaning.

Snapshots can be saved and reopened. Objects and IDs obtained from an index expire when it closes. Use try-with-resources to release native memory.

Name lookup uses an ASCII representation. Unsupported non-ASCII member names are excluded; non-ASCII generic signatures use erased descriptors. String-literal queries preserve UTF-16 values, including non-ASCII text. A semantic literal occurrence differs from an arbitrary constant-pool entry. See [terminology](CONTEXT.md).

Snapshots are generated data tied to the current format. Older formats require rebuilding. Treat snapshot files as trusted application caches, not arbitrary user-supplied imports.

## Build

Install a full JDK 21 and the Rust toolchain pinned in [rust-toolchain.toml](jindex-rs/rust-toolchain.toml), including Clippy and rustfmt. Use the checked-in wrapper:

```powershell
.\gradlew.bat build publishToMavenLocal --warning-mode fail
```

The build compiles the locked native dependencies, packages the DLL and Java API, and runs Java tests, native tests, rustfmt and Clippy. Push and pull-request checks run on Windows.

The default version is defined in [build.gradle](build.gradle). Override it with `-PjindexVersion=<version>`. For local builds, add Maven Local explicitly in the consuming application.

Published artifacts use `com.github.tth05:jindex` from Packagecloud:

```groovy
repositories {
    maven { url = uri('https://packagecloud.io/tth05/repo/maven2') }
}
dependencies {
    implementation 'com.github.tth05:jindex:<published-version>'
}
```

## Corpus checks

The test sources include a runtime-corpus benchmark and an independent ASM member-reference check. These require external class archives and run separately from ordinary CI. Their Gradle tasks and input properties are defined in [build.gradle](build.gradle).

Measure cold construction, persistence, warm loading and query latency separately. Keep the source manifest, Java runtime and selected class counts fixed when comparing results. Whole-process memory includes Java inputs and query objects as well as the native index.
