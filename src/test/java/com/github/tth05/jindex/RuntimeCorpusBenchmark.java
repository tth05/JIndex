package com.github.tth05.jindex;

import java.io.IOException;
import java.net.URI;
import java.nio.file.FileSystem;
import java.nio.file.FileSystems;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.time.Instant;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HexFormat;
import java.util.EnumSet;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Set;
import java.util.function.Supplier;
import java.util.stream.Stream;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;
import java.util.zip.ZipOutputStream;

/** Records the mixed-source JIndex baseline against a TotalDebug runtime-source manifest. */
public final class RuntimeCorpusBenchmark {
    private static final String MANIFEST_HEADER = "totaldebug-runtime-sources-v1";
    private static final int REFERENCE_QUERY_LIMIT = 200;
    private static volatile Object blackhole;

    private RuntimeCorpusBenchmark() {
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length != 2) {
            throw new IllegalArgumentException("Expected <runtime-source-manifest> <result-json>");
        }

        Path manifest = Path.of(arguments[0]).toAbsolutePath().normalize();
        Path resultFile = Path.of(arguments[1]).toAbsolutePath().normalize();
        List<Path> sources = readManifest(manifest);
        long inventoryStarted = System.nanoTime();
        CorpusInventory inventory = inventory(sources);
        long inventoryNanos = elapsedNanos(inventoryStarted);

        long sourcePreparationStarted = System.nanoTime();
        LooseClasses jdkClasses = readJdkClasses();
        long sourcePreparationNanos = elapsedNanos(sourcePreparationStarted);
        String oracleJdkArchive = System.getenv("JINDEX_ORACLE_JDK_ARCHIVE");
        if (oracleJdkArchive != null) {
            // Preserve the exact runtime-image class bytes/order for the native differential test.
            try (var archive = new ZipOutputStream(Files.newOutputStream(Path.of(oracleJdkArchive)))) {
                for (int index = 0; index < jdkClasses.bytes().size(); index++) {
                    archive.putNextEntry(new ZipEntry(index + ".class"));
                    archive.write(jdkClasses.bytes().get(index));
                    archive.closeEntry();
                }
            }
        }

        Path workspace = Files.createTempDirectory("jindex-runtime-corpus-");
        try {
            Path persistedIndex = workspace.resolve("index");
            long buildStarted = System.nanoTime();
            BuildTimeInfo nativeBuildTimes;
            QueryMeasurements queryMeasurements;
            try (ClassIndex index = ClassIndex.fromSources(
                    sources.stream().map(Path::toString).toList(),
                    jdkClasses.bytes()
            )) {
                long buildNanos = elapsedNanos(buildStarted);
                nativeBuildTimes = index.getBuildTimeInfo();

                requireClass(index, "net/minecraft/world/level/block", "Block");
                requireClass(index, "java/lang", "String");

                queryMeasurements = measureQueries(index);
                IndexStatistics statistics = index.getStatistics();
                ReferenceStorageStatistics referenceStorage = index.getReferenceStorageStatistics();

                long saveStarted = System.nanoTime();
                index.saveToFile(persistedIndex.toString());
                long saveNanos = elapsedNanos(saveStarted);

                LoadMeasurements loadMeasurements = measureLoads(persistedIndex);
                String json = json(
                        manifest,
                        sources,
                        inventory,
                        inventoryNanos,
                        jdkClasses,
                        sourcePreparationNanos,
                        buildNanos,
                        nativeBuildTimes,
                        statistics,
                        referenceStorage,
                        saveNanos,
                        Files.size(persistedIndex),
                        loadMeasurements,
                        queryMeasurements
                );
                Path resultDirectory = resultFile.getParent();
                if (resultDirectory != null) {
                    Files.createDirectories(resultDirectory);
                }
                Files.writeString(resultFile, json, StandardCharsets.UTF_8);
                System.out.println("RESULT " + resultFile);
            }
        } finally {
            deleteTree(workspace);
        }
    }

    private static List<Path> readManifest(Path manifest) throws IOException {
        List<String> lines = Files.readAllLines(manifest, StandardCharsets.UTF_8);
        if (lines.isEmpty() || !MANIFEST_HEADER.equals(lines.getFirst())) {
            throw new IOException("Unsupported runtime-source manifest: " + manifest);
        }
        List<Path> sources = new ArrayList<>(lines.size() - 1);
        for (int index = 1; index < lines.size(); index++) {
            URI uri = URI.create(lines.get(index));
            if (!"file".equalsIgnoreCase(uri.getScheme())) {
                throw new IOException("Runtime source is not a file URI at line " + (index + 1));
            }
            Path source = Path.of(uri).toAbsolutePath().normalize();
            if (!Files.isRegularFile(source)) {
                throw new IOException("Runtime source is not a regular file: " + source);
            }
            sources.add(source);
        }
        if (sources.isEmpty()) {
            throw new IOException("Runtime-source manifest is empty: " + manifest);
        }
        return List.copyOf(sources);
    }

    private static CorpusInventory inventory(List<Path> sources) throws IOException {
        long sourceBytes = 0;
        long classBytes = 0;
        int classFiles = 0;
        for (Path source : sources) {
            sourceBytes = Math.addExact(sourceBytes, Files.size(source));
            try (ZipFile archive = new ZipFile(source.toFile())) {
                var entries = archive.entries();
                while (entries.hasMoreElements()) {
                    ZipEntry entry = entries.nextElement();
                    if (entry.isDirectory()
                            || !entry.getName().endsWith(".class")
                            || entry.getName().equals("module-info.class")) {
                        continue;
                    }
                    classFiles++;
                    if (entry.getSize() >= 0) {
                        classBytes = Math.addExact(classBytes, entry.getSize());
                    }
                }
            }
        }
        return new CorpusInventory(sourceBytes, classFiles, classBytes);
    }

    private static LooseClasses readJdkClasses() throws IOException {
        List<byte[]> classes = new ArrayList<>();
        Set<String> entries = new LinkedHashSet<>();
        long classBytes = 0;
        FileSystem jrt = FileSystems.getFileSystem(URI.create("jrt:/"));
        Path modules = jrt.getPath("/modules");
        try (Stream<Path> modulePaths = Files.list(modules)) {
            for (Path module : modulePaths.sorted().toList()) {
                try (Stream<Path> moduleClasses = Files.walk(module)) {
                    for (Path classFile : moduleClasses.filter(Files::isRegularFile).sorted().toList()) {
                        String entryName = module.relativize(classFile).toString().replace('\\', '/');
                        if (!isIndexableClassEntry(entryName) || !entries.add(entryName)) {
                            continue;
                        }
                        byte[] bytes = Files.readAllBytes(classFile);
                        classes.add(bytes);
                        classBytes = Math.addExact(classBytes, bytes.length);
                    }
                }
            }
        }
        return new LooseClasses(List.copyOf(classes), classBytes);
    }

    private static boolean isIndexableClassEntry(String entryName) {
        return entryName.endsWith(".class")
                && !entryName.equals("module-info.class")
                && !entryName.endsWith("/module-info.class");
    }

    private static QueryMeasurements measureQueries(ClassIndex index) {
        SearchOptions prefix = SearchOptions.with(
                SearchOptions.SearchMode.PREFIX,
                SearchOptions.MatchMode.IGNORE_CASE,
                200
        );
        SearchOptions contains = SearchOptions.with(
                SearchOptions.SearchMode.CONTAINS,
                SearchOptions.MatchMode.IGNORE_CASE,
                200
        );
        Measurement exactClass = measure(25, 1_000, () -> index.findClass(
                "net/minecraft/world/level/block",
                "Block"
        ));
        Measurement prefixClasses = measure(15, 200, () -> index.findClasses("Block", prefix));
        Measurement containsClasses = measure(15, 200, () -> index.findClasses("block", contains));
        Measurement exactMethod = measure(15, 500, () -> index.findSymbols(
                "defaultBlockState",
                prefix,
                EnumSet.of(SymbolKind.METHOD)
        ));
        Measurement prefixSymbols = measure(15, 200, () -> index.findSymbols(
                "get",
                prefix,
                EnumSet.of(SymbolKind.FIELD, SymbolKind.METHOD)
        ));
        Measurement containsSymbols = measure(15, 200, () -> index.findSymbols(
                "block",
                contains,
                EnumSet.of(SymbolKind.FIELD, SymbolKind.METHOD)
        ));
        ReferenceMeasurement classReferences = measureReferences(
                index,
                ReferenceTarget.classTarget("net/minecraft/world/level/block/Block")
        );
        ReferenceMeasurement fieldReferences = measureReferences(
                index,
                ReferenceTarget.fieldTarget(
                        "net/minecraft/world/level/block/Blocks",
                        "AIR",
                        "Lnet/minecraft/world/level/block/Block;"
                )
        );
        ReferenceMeasurement methodReferences = measureReferences(
                index,
                ReferenceTarget.methodTarget(
                        "net/minecraft/world/level/block/Block",
                        "defaultBlockState",
                        "()Lnet/minecraft/world/level/block/state/BlockState;"
                )
        );
        ReferenceMeasurement literalReferences = measureLiteralReferences(index, "minecraft");
        Measurement containsLiterals = measure(
                10,
                200,
                () -> index.findLiteralsContaining("block", 200)
        );
        return new QueryMeasurements(
                exactClass,
                prefixClasses,
                containsClasses,
                exactMethod,
                prefixSymbols,
                containsSymbols,
                classReferences,
                fieldReferences,
                methodReferences,
                literalReferences,
                containsLiterals
        );
    }

    private static ReferenceMeasurement measureReferences(ClassIndex index, ReferenceTarget target) {
        ReferenceSearchPage all = index.findReferences(target, Integer.MAX_VALUE);
        int resultCount = all.results().length;
        if (resultCount == 0) {
            throw new IllegalStateException("Benchmark reference target has no usages: " + target);
        }
        return new ReferenceMeasurement(
                resultCount,
                measure(10, 200, () -> index.findReferences(target, REFERENCE_QUERY_LIMIT))
        );
    }

    private static ReferenceMeasurement measureLiteralReferences(ClassIndex index, String literal) {
        ReferenceSearchPage all = index.findLiteralReferences(literal, Integer.MAX_VALUE);
        int resultCount = all.results().length;
        if (resultCount == 0) {
            throw new IllegalStateException("Benchmark literal has no usages: " + literal);
        }
        return new ReferenceMeasurement(
                resultCount,
                measure(10, 200, () -> index.findLiteralReferences(literal, REFERENCE_QUERY_LIMIT))
        );
    }

    private static Measurement measure(int warmups, int samples, Supplier<?> operation) {
        for (int index = 0; index < warmups; index++) {
            blackhole = operation.get();
        }
        long[] nanos = new long[samples];
        for (int index = 0; index < samples; index++) {
            long started = System.nanoTime();
            blackhole = operation.get();
            nanos[index] = elapsedNanos(started);
        }
        Arrays.sort(nanos);
        return new Measurement(
                nanos[0],
                percentile(nanos, 0.50),
                percentile(nanos, 0.95),
                nanos[nanos.length - 1],
                samples
        );
    }

    private static LoadMeasurements measureLoads(Path persistedIndex) {
        long[] nanos = new long[6];
        BuildTimeInfo firstInfo = null;
        for (int index = 0; index < nanos.length; index++) {
            long started = System.nanoTime();
            try (ClassIndex loaded = ClassIndex.fromFile(persistedIndex.toString())) {
                nanos[index] = elapsedNanos(started);
                requireClass(loaded, "net/minecraft/world/level/block", "Block");
                if (index == 0) {
                    firstInfo = loaded.getBuildTimeInfo();
                }
            }
        }
        long first = nanos[0];
        long[] warm = Arrays.copyOfRange(nanos, 1, nanos.length);
        Arrays.sort(warm);
        return new LoadMeasurements(
                first,
                percentile(warm, 0.50),
                percentile(warm, 0.95),
                firstInfo == null ? 0 : firstInfo.getClassReadingTime(),
                firstInfo == null ? 0 : firstInfo.getDeserializationTime()
        );
    }

    private static long percentile(long[] sorted, double percentile) {
        int index = (int) Math.ceil(percentile * sorted.length) - 1;
        return sorted[Math.max(0, Math.min(sorted.length - 1, index))];
    }

    private static String json(
            Path manifest,
            List<Path> sources,
            CorpusInventory inventory,
            long inventoryNanos,
            LooseClasses jdkClasses,
            long sourcePreparationNanos,
            long buildNanos,
            BuildTimeInfo nativeBuildTimes,
            IndexStatistics statistics,
            ReferenceStorageStatistics referenceStorage,
            long saveNanos,
            long persistedBytes,
            LoadMeasurements loads,
            QueryMeasurements queries
    ) throws IOException, NoSuchAlgorithmException {
        return """
                {
                  "schema": "jindex-runtime-baseline-v1",
                  "recordedAt": "%s",
                  "commit": "%s",
                  "javaVersion": "%s",
                  "javaHome": "%s",
                  "processors": %d,
                  "manifest": "%s",
                  "manifestSha256": "%s",
                  "archiveSourceCount": %d,
                  "archiveSourceBytes": %d,
                  "archiveClassFileCount": %d,
                  "archiveClassFileBytes": %d,
                  "jdkClassFileCount": %d,
                  "jdkClassFileBytes": %d,
                  "classInputCount": %d,
                  "inventoryNanos": %d,
                  "sourcePreparationNanos": %d,
                  "buildNanos": %d,
                  "nativeClassReadingMillis": %d,
                  "nativeIndexingMillis": %d,
                  "selectedClassCount": %d,
                  "fieldCount": %d,
                  "methodCount": %d,
                  "referenceSiteCount": %d,
                  "referenceOccurrenceCount": %d,
                  "singleOccurrenceReferenceSiteCount": %d,
                  "referenceCountOver255SiteCount": %d,
                  "referenceCountOver65535SiteCount": %d,
                  "maximumReferenceOccurrenceCount": %d,
                  "literalCount": %d,
                  "literalOccurrenceCount": %d,
                  "saveNanos": %d,
                  "persistedBytes": %d,
                  "firstLoadNanos": %d,
                  "warmLoadP50Nanos": %d,
                  "warmLoadP95Nanos": %d,
                  "firstLoadReadMillis": %d,
                  "firstLoadDeserializeMillis": %d,
                  "queries": {
                    "exactClass": %s,
                    "prefixClasses": %s,
                    "containsClasses": %s,
                    "exactMethod": %s,
                    "prefixSymbols": %s,
                    "containsSymbols": %s,
                    "classReferences": %s,
                    "fieldReferences": %s,
                    "methodReferences": %s,
                    "literalReferences": %s,
                    "containsLiterals": %s
                  }
                }
                """.formatted(
                Instant.now(),
                escape(System.getenv().getOrDefault("JINDEX_BENCHMARK_COMMIT", "unknown")),
                escape(System.getProperty("java.version")),
                escape(System.getProperty("java.home")),
                Runtime.getRuntime().availableProcessors(),
                escape(manifest.toString()),
                sha256(manifest),
                sources.size(),
                inventory.sourceBytes(),
                inventory.classFiles(),
                inventory.classBytes(),
                jdkClasses.bytes().size(),
                jdkClasses.classBytes(),
                Math.addExact(inventory.classFiles(), jdkClasses.bytes().size()),
                inventoryNanos,
                sourcePreparationNanos,
                buildNanos,
                nativeBuildTimes.getClassReadingTime(),
                nativeBuildTimes.getIndexingTime(),
                statistics.classCount(),
                statistics.fieldCount(),
                statistics.methodCount(),
                statistics.referenceSiteCount(),
                referenceStorage.occurrenceCount(),
                referenceStorage.singleOccurrenceSiteCount(),
                referenceStorage.countOver255SiteCount(),
                referenceStorage.countOver65535SiteCount(),
                referenceStorage.maximumOccurrenceCount(),
                statistics.literalCount(),
                statistics.literalOccurrenceCount(),
                saveNanos,
                persistedBytes,
                loads.firstNanos(),
                loads.warmP50Nanos(),
                loads.warmP95Nanos(),
                loads.firstReadMillis(),
                loads.firstDeserializeMillis(),
                queries.exactClass().json(),
                queries.prefixClasses().json(),
                queries.containsClasses().json(),
                queries.exactMethod().json(),
                queries.prefixSymbols().json(),
                queries.containsSymbols().json(),
                queries.classReferences().json(),
                queries.fieldReferences().json(),
                queries.methodReferences().json(),
                queries.literalReferences().json(),
                queries.containsLiterals().json()
        );
    }

    private static String sha256(Path path) throws IOException, NoSuchAlgorithmException {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(Files.readAllBytes(path)));
    }

    private static String escape(String value) {
        StringBuilder escaped = new StringBuilder(value.length() + 16);
        for (int index = 0; index < value.length(); index++) {
            char character = value.charAt(index);
            switch (character) {
                case '\\' -> escaped.append("\\\\");
                case '"' -> escaped.append("\\\"");
                case '\n' -> escaped.append("\\n");
                case '\r' -> escaped.append("\\r");
                case '\t' -> escaped.append("\\t");
                default -> {
                    if (character < 0x20) {
                        escaped.append("\\u%04x".formatted(Locale.ROOT, (int) character));
                    } else {
                        escaped.append(character);
                    }
                }
            }
        }
        return escaped.toString();
    }

    private static long elapsedNanos(long started) {
        return System.nanoTime() - started;
    }

    private static void requireClass(ClassIndex index, String packageName, String className) {
        if (index.findClass(packageName, className) == null) {
            throw new IllegalStateException("Missing class " + packageName + "/" + className);
        }
    }

    private static void deleteTree(Path root) throws IOException {
        if (!Files.exists(root)) {
            return;
        }
        try (var paths = Files.walk(root)) {
            for (Path path : paths.sorted((left, right) -> right.compareTo(left)).toList()) {
                Files.deleteIfExists(path);
            }
        }
    }

    private record CorpusInventory(long sourceBytes, int classFiles, long classBytes) {
    }

    private record LooseClasses(List<byte[]> bytes, long classBytes) {
    }

    private record Measurement(long minNanos, long p50Nanos, long p95Nanos, long maxNanos, int samples) {
        private String json() {
            return """
                    {"samples":%d,"minNanos":%d,"p50Nanos":%d,"p95Nanos":%d,"maxNanos":%d}
                    """.formatted(this.samples, this.minNanos, this.p50Nanos, this.p95Nanos, this.maxNanos).trim();
        }
    }

    private record LoadMeasurements(
            long firstNanos,
            long warmP50Nanos,
            long warmP95Nanos,
            long firstReadMillis,
            long firstDeserializeMillis
    ) {
    }

    private record QueryMeasurements(
            Measurement exactClass,
            Measurement prefixClasses,
            Measurement containsClasses,
            Measurement exactMethod,
            Measurement prefixSymbols,
            Measurement containsSymbols,
            ReferenceMeasurement classReferences,
            ReferenceMeasurement fieldReferences,
            ReferenceMeasurement methodReferences,
            ReferenceMeasurement literalReferences,
            Measurement containsLiterals
    ) {
    }

    private record ReferenceMeasurement(int resultCount, Measurement latency) {
        private String json() {
            return "{\"resultCount\":%d,\"latency\":%s}".formatted(this.resultCount, this.latency.json());
        }
    }
}
