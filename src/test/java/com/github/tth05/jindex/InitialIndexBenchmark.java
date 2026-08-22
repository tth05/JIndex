package com.github.tth05.jindex;

import java.io.IOException;
import java.io.OutputStream;
import java.net.URI;
import java.nio.file.FileSystem;
import java.nio.file.FileSystems;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import java.util.stream.Stream;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;

/**
 * Reproduces the expensive synthetic-JDK-JAR part of TotalDebug's initial class-index build.
 * This is deliberately a standalone benchmark so normal unit tests stay fast.
 */
public final class InitialIndexBenchmark {
    private InitialIndexBenchmark() {
    }

    public static void main(String[] args) throws Exception {
        String mode = args.length == 0 ? "legacy-jdk" : args[0];
        if (!mode.equals("legacy-jdk") && !mode.equals("mixed-jdk")) {
            throw new IllegalArgumentException("Unknown benchmark mode: " + mode);
        }

        Path workspace = Files.createTempDirectory("jindex-initial-benchmark-");
        try {
            if (mode.equals("legacy-jdk")) {
                runLegacyJdkBenchmark(workspace);
            } else {
                runMixedJdkBenchmark(workspace);
            }
        } finally {
            deleteTree(workspace);
        }
    }

    private static void runMixedJdkBenchmark(Path workspace) throws IOException {
        long totalStarted = System.nanoTime();

        long prepareStarted = System.nanoTime();
        List<byte[]> classes = readJdkClasses();
        long prepareMillis = elapsedMillis(prepareStarted);

        long buildStarted = System.nanoTime();
        try (ClassIndex index = ClassIndex.fromSources(List.of(), classes)) {
            long buildMillis = elapsedMillis(buildStarted);
            verifyRepresentativeClasses(index);

            Path outputIndex = workspace.resolve("index");
            long saveStarted = System.nanoTime();
            index.saveToFile(outputIndex.toString());
            long saveMillis = elapsedMillis(saveStarted);

            System.out.printf(
                    "RESULT mode=mixed-jdk classes=%d verifiedClasses=3 prepareMs=%d buildMs=%d saveMs=%d totalMs=%d outputBytes=%d%n",
                    classes.size(),
                    prepareMillis,
                    buildMillis,
                    saveMillis,
                    elapsedMillis(totalStarted),
                    Files.size(outputIndex)
            );
        }
    }

    private static void runLegacyJdkBenchmark(Path workspace) throws IOException {
        long totalStarted = System.nanoTime();
        Path inputJar = workspace.resolve("jdk-classes.jar");

        long prepareStarted = System.nanoTime();
        int classCount = packJdkClasses(inputJar);
        long prepareMillis = elapsedMillis(prepareStarted);

        long buildStarted = System.nanoTime();
        try (ClassIndex index = ClassIndex.fromJars(List.of(inputJar.toString()))) {
            long buildMillis = elapsedMillis(buildStarted);
            verifyRepresentativeClasses(index);

            Path outputIndex = workspace.resolve("index");
            long saveStarted = System.nanoTime();
            index.saveToFile(outputIndex.toString());
            long saveMillis = elapsedMillis(saveStarted);

            System.out.printf(
                    "RESULT mode=legacy-jdk classes=%d prepareMs=%d buildMs=%d saveMs=%d totalMs=%d outputBytes=%d%n",
                    classCount,
                    prepareMillis,
                    buildMillis,
                    saveMillis,
                    elapsedMillis(totalStarted),
                    Files.size(outputIndex)
            );
        }
    }

    private static int packJdkClasses(Path outputJar) throws IOException {
        int classCount = 0;
        Set<String> entries = new LinkedHashSet<>();
        FileSystem jrt = FileSystems.getFileSystem(URI.create("jrt:/"));
        Path modules = jrt.getPath("/modules");
        try (OutputStream output = Files.newOutputStream(outputJar);
             ZipOutputStream zip = new ZipOutputStream(output);
             Stream<Path> modulePaths = Files.list(modules)) {
            for (Path module : modulePaths.sorted().toList()) {
                try (Stream<Path> classes = Files.walk(module)) {
                    for (Path classFile : classes.filter(Files::isRegularFile).sorted().toList()) {
                        String entryName = module.relativize(classFile).toString().replace('\\', '/');
                        if (!isIndexableClassEntry(entryName) || !entries.add(entryName)) {
                            continue;
                        }
                        zip.putNextEntry(new ZipEntry(entryName));
                        Files.copy(classFile, zip);
                        zip.closeEntry();
                        classCount++;
                    }
                }
            }
        }
        return classCount;
    }

    private static List<byte[]> readJdkClasses() throws IOException {
        List<byte[]> classes = new ArrayList<>();
        Set<String> entries = new LinkedHashSet<>();
        FileSystem jrt = FileSystems.getFileSystem(URI.create("jrt:/"));
        Path modules = jrt.getPath("/modules");
        try (Stream<Path> modulePaths = Files.list(modules)) {
            for (Path module : modulePaths.sorted().toList()) {
                try (Stream<Path> moduleClasses = Files.walk(module)) {
                    for (Path classFile : moduleClasses.filter(Files::isRegularFile).sorted().toList()) {
                        String entryName = module.relativize(classFile).toString().replace('\\', '/');
                        if (isIndexableClassEntry(entryName) && entries.add(entryName)) {
                            classes.add(Files.readAllBytes(classFile));
                        }
                    }
                }
            }
        }
        return classes;
    }

    private static boolean isIndexableClassEntry(String entryName) {
        return entryName.endsWith(".class")
                && !entryName.equals("module-info.class")
                && !entryName.endsWith("/module-info.class");
    }

    private static void verifyRepresentativeClasses(ClassIndex index) {
        requireClass(index, "java/lang", "String");
        requireClass(index, "java/util", "List");
        requireClass(index, "jdk/internal/misc", "Unsafe");
    }

    private static void requireClass(ClassIndex index, String packageName, String className) {
        if (index.findClass(packageName, className) == null) {
            throw new IllegalStateException("Missing class " + packageName + "/" + className);
        }
    }

    private static long elapsedMillis(long started) {
        return (System.nanoTime() - started) / 1_000_000;
    }

    private static void deleteTree(Path root) throws IOException {
        if (!Files.exists(root)) {
            return;
        }
        try (Stream<Path> paths = Files.walk(root)) {
            for (Path path : paths.sorted(Comparator.reverseOrder()).toList()) {
                Files.deleteIfExists(path);
            }
        }
    }
}
