package com.github.tth05.jindex;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Objects;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.*;

final class NativeLibraryCacheTest {
    @TempDir Path directory;

    @Test
    void identicalContentsReuseOneVerifiedFileEvenWithConcurrentCallers() throws Exception {
        byte[] bytes = "native-library-fixture".getBytes(StandardCharsets.UTF_8);
        try (var executor = Executors.newFixedThreadPool(8)) {
            var futures = new ArrayList<java.util.concurrent.Future<Path>>();
            for (int index = 0; index < 32; index++) {
                futures.add(executor.submit(() -> NativeLibraryLoader.extract(bytes, directory)));
            }
            Path expected = futures.getFirst().get(5, TimeUnit.SECONDS);
            for (var future : futures) {
                assertEquals(expected, future.get(5, TimeUnit.SECONDS));
            }
            assertArrayEquals(bytes, Files.readAllBytes(expected));
            assertNotEquals(expected, NativeLibraryLoader.extract(new byte[]{1, 2}, directory));
        }
        try (var files = Files.list(directory)) {
            assertEquals(2, files.filter(path -> path.toString().endsWith(".dll")).count());
        }
    }

    @Test
    void aModifiedCachedLibraryFailsInsteadOfBeingLoadedOrOverwritten() throws Exception {
        byte[] bytes = {1, 2, 3};
        Path cached = NativeLibraryLoader.extract(bytes, directory);
        Files.writeString(cached, "corrupted");
        IOException failure = assertThrows(IOException.class,
                () -> NativeLibraryLoader.extract(bytes, directory));
        assertTrue(failure.getMessage().contains("checksum mismatch"));
        assertEquals("corrupted", Files.readString(cached));
    }

    @Test
    void separateJvmsLoadTheSameCachedDll() throws Exception {
        Path mainClasses = Path.of(ClassIndex.class.getProtectionDomain().getCodeSource().getLocation().toURI());
        Path testClasses = Path.of(getClass().getProtectionDomain().getCodeSource().getLocation().toURI());
        Path resources = Path.of(Objects.requireNonNull(ClassIndex.class.getResource("/jindex_natives/jindex_rs.dll")).toURI())
                .getParent().getParent();
        String classPath = String.join(java.io.File.pathSeparator,
                mainClasses.toString(), testClasses.toString(), resources.toString());
        var command = List.of(Path.of(System.getProperty("java.home"), "bin", "java.exe").toString(),
                "-Djindex.native.cacheDir=" + directory, "-cp", classPath, Probe.class.getName());
        var first = new ProcessBuilder(command).redirectErrorStream(true).redirectOutput(directory.resolve("first.log").toFile()).start();
        var second = new ProcessBuilder(command).redirectErrorStream(true).redirectOutput(directory.resolve("second.log").toFile()).start();
        try {
            for (Process process : List.of(first, second)) {
                if (!process.waitFor(30, TimeUnit.SECONDS)) {
                    fail("Native cache probe did not finish");
                }
                assertEquals(0, process.exitValue(), () -> assertDoesNotThrow(() ->
                        Files.readString(directory.resolve("first.log")) + Files.readString(directory.resolve("second.log"))));
            }
        } finally {
            if (first.isAlive()) first.destroyForcibly();
            if (second.isAlive()) second.destroyForcibly();
        }
        List<Path> libraries;
        try (var files = Files.list(directory)) {
            libraries = files.filter(path -> path.toString().endsWith(".dll")).toList();
        }
        // Each JVM loads JIndex in its app loader and two isolated loaders. Slots are reused across JVMs.
        assertEquals(3, libraries.size());
        // Windows may briefly retain the image handle after process exit. Do not weaken the load assertions.
        for (Path library : libraries) {
            long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5);
            while (true) {
                try {
                    Files.delete(library);
                    break;
                } catch (java.nio.file.AccessDeniedException exception) {
                    if (System.nanoTime() >= deadline) throw exception;
                    Thread.sleep(10);
                }
            }
        }
    }

    public static final class Probe {
        public static void main(String[] arguments) throws Exception {
            byte[] fixture;
            try (var resource = Objects.requireNonNull(Probe.class.getResourceAsStream("/" + Probe.class.getName().replace('.', '/') + ".class"));
                 var index = ClassIndex.fromBytes(List.of(resource.readAllBytes()))) {
                if (index.findClass(Probe.class.getName()) == null) {
                    throw new AssertionError("Native index did not load");
                }
            }
            try (var resource = Objects.requireNonNull(Probe.class.getResourceAsStream("/" + Probe.class.getName().replace('.', '/') + ".class"))) {
                fixture = resource.readAllBytes();
            }
            var classes = ClassIndex.class.getProtectionDomain().getCodeSource().getLocation();
            var resources = Path.of(Objects.requireNonNull(ClassIndex.class.getResource("/jindex_natives/jindex_rs.dll")).toURI())
                    .getParent().getParent().toUri().toURL();
            for (int ordinal = 0; ordinal < 2; ordinal++) {
                try (var loader = new java.net.URLClassLoader(new java.net.URL[]{classes, resources}, null)) {
                    Class<?> indexType = Class.forName(ClassIndex.class.getName(), true, loader);
                    try (var index = (AutoCloseable) indexType.getMethod("fromBytes", List.class).invoke(null, List.of(fixture))) {
                        if (indexType.getMethod("findClass", String.class).invoke(index, Probe.class.getName()) == null) {
                            throw new AssertionError("Isolated class loader did not receive its own native field IDs");
                        }
                    }
                }
            }
        }
    }
}
