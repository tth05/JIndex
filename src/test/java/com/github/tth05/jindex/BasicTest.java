package com.github.tth05.jindex;

import org.junit.jupiter.api.AfterAll;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestInstance;
import org.junit.jupiter.api.function.Executable;

import java.io.InputStream;
import java.lang.reflect.Modifier;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Arrays;
import java.util.Collections;
import java.util.Comparator;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;
import java.util.stream.Stream;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;

import javax.tools.ToolProvider;

import static org.junit.jupiter.api.Assertions.*;

@TestInstance(TestInstance.Lifecycle.PER_CLASS)
public class BasicTest {

    private ClassIndex index;

    @BeforeAll
    public void init() {
        SampleClassesHelper.createSamplesJar();
        try (ClassIndex tempIndex = ClassIndex.fromJars(Collections.singletonList("src/test/resources/Samples.jar"))) {
            tempIndex.saveToFile("index");
            System.out.println(tempIndex.getBuildTimeInfo().toFormattedString());
        }
        this.index = ClassIndex.fromFile("index");
    }

    @AfterAll
    public void cleanup() {
        this.index.close();
        assertDoesNotThrow(() -> Files.deleteIfExists(Paths.get("index")));
    }

    @Test
    public void testFindClass() {
        IndexedClass singleClass = index.findClass("java/lang", "String");
        assertNotNull(singleClass);
        assertEquals("java/lang/String", singleClass.getNameWithPackage());
    }

    @Test
    public void testBuildFromBytes() {
        try (ClassIndex byteIndex = ClassIndex.fromBytes(SampleClassesHelper.loadSampleClasses())) {
            assertNotNull(byteIndex.findClass("java/lang", "String"));
        }
    }

    @Test
    public void testBuildFromMixedSources() throws Exception {
        Path archive = Files.createTempFile("jindex-mixed-sources-", ".jar");
        try {
            byte[] stringClass = readClassBytes(String.class);
            try (ZipOutputStream output = new ZipOutputStream(Files.newOutputStream(archive))) {
                output.putNextEntry(new ZipEntry("java/lang/String.class"));
                output.write(stringClass);
                output.closeEntry();
            }

            try (ClassIndex mixedIndex = ClassIndex.fromSources(
                    List.of(archive.toString()),
                    List.of(readClassBytes(java.util.ArrayList.class))
            )) {
                assertNotNull(mixedIndex.findClass("java/lang", "String"));
                assertNotNull(mixedIndex.findClass("java/util", "ArrayList"));
            }
        } finally {
            Files.deleteIfExists(archive);
        }
    }

    @Test
    public void testDirectClassesOverrideArchiveClasses() throws Exception {
        Path workspace = Files.createTempDirectory("jindex-source-precedence-");
        try {
            byte[] archiveClass = compileFixture(
                    workspace.resolve("archive-compile"),
                    "package mixed; public class Fixture { public int archiveField; }"
            );
            byte[] directClass = compileFixture(
                    workspace.resolve("direct-compile"),
                    "package mixed; public class Fixture { public int directField; }"
            );
            Path archive = workspace.resolve("fixture.jar");
            try (ZipOutputStream output = new ZipOutputStream(Files.newOutputStream(archive))) {
                output.putNextEntry(new ZipEntry("mixed/Fixture.class"));
                output.write(archiveClass);
                output.closeEntry();
            }

            try (ClassIndex mixedIndex = ClassIndex.fromSources(
                    List.of(archive.toString()),
                    List.of(directClass)
            )) {
                IndexedClass fixture = mixedIndex.findClass("mixed", "Fixture");
                assertNotNull(fixture);
                assertTrue(Arrays.stream(fixture.getFields()).anyMatch(field -> field.getName().equals("directField")));
                assertFalse(Arrays.stream(fixture.getFields()).anyMatch(field -> field.getName().equals("archiveField")));
            }
        } finally {
            deleteTree(workspace);
        }
    }

    @Test
    public void testCloseIsIdempotentAndGuardsIndexOperations() {
        ClassIndex closedIndex = ClassIndex.fromJars(Collections.singletonList("src/test/resources/Samples.jar"));
        closedIndex.close();
        assertDoesNotThrow(closedIndex::close);
        assertTrue(closedIndex.isDestroyed());
        assertClosed(() -> closedIndex.findClass("java/lang", "String"));
    }

    @Test
    public void testRetainedChildrenAreGuardedAfterClose() {
        ClassIndex closedIndex = ClassIndex.fromJars(Collections.singletonList("src/test/resources/Samples.jar"));
        try {
            IndexedClass indexedClass = closedIndex.findClass("java/lang", "String");
            assertNotNull(indexedClass);
            IndexedPackage indexedPackage = indexedClass.getPackage();
            IndexedField indexedField = indexedClass.getFields()[0];
            IndexedMethod indexedMethod = indexedClass.getMethods()[0];

            closedIndex.close();

            assertClosed(indexedClass::getName);
            assertClosed(indexedPackage::getName);
            assertClosed(indexedField::getName);
            assertClosed(indexedMethod::getName);
        } finally {
            closedIndex.close();
        }
    }

    @Test
    public void testCloseWaitsForInFlightOperation() throws Exception {
        ClassIndex concurrentIndex = ClassIndex.fromJars(Collections.singletonList("src/test/resources/Samples.jar"));
        CountDownLatch operationStarted = new CountDownLatch(1);
        CountDownLatch releaseOperation = new CountDownLatch(1);
        CountDownLatch closeStarted = new CountDownLatch(1);
        ExecutorService executor = Executors.newFixedThreadPool(2);
        Future<?> operation = executor.submit(() -> concurrentIndex.executeWhileOpen(() -> {
            operationStarted.countDown();
            await(releaseOperation);
        }));

        try {
            assertTrue(operationStarted.await(5, TimeUnit.SECONDS));
            Future<?> close = executor.submit(() -> {
                closeStarted.countDown();
                concurrentIndex.close();
            });

            assertTrue(closeStarted.await(5, TimeUnit.SECONDS));
            assertThrows(TimeoutException.class, () -> close.get(250, TimeUnit.MILLISECONDS));

            releaseOperation.countDown();
            operation.get(5, TimeUnit.SECONDS);
            close.get(5, TimeUnit.SECONDS);
            assertTrue(concurrentIndex.isDestroyed());
            assertClosed(() -> concurrentIndex.findClass("java/lang", "String"));
        } finally {
            releaseOperation.countDown();
            concurrentIndex.close();
            executor.shutdownNow();
            assertTrue(executor.awaitTermination(5, TimeUnit.SECONDS));
        }
    }

    @Test
    public void testFindClasses() {
        IndexedClass[] results = index.findClasses("String", SearchOptions.defaultOptions());
        for (IndexedClass result : results)
            assertTrue(result.getName().startsWith("String"));

        IndexedClass resultClass = Arrays.stream(results).filter(c -> c.getNameWithPackage().equals("java/lang/String")).findFirst().get();
        assertTrue(resultClass.getFields().length > 0);
        assertTrue(resultClass.getMethods().length > 0);
        assertEquals("java/lang/String", resultClass.getNameWithPackage());
        assertTrue(Modifier.isPublic(resultClass.getAccessFlags()));

        var serialVersionUid = Arrays.stream(resultClass.getFields())
                .filter(field -> field.getName().equals("serialVersionUID"))
                .findFirst()
                .orElseThrow();
        assertTrue(Modifier.isStatic(serialVersionUid.getAccessFlags()));
        assertTrue(Modifier.isFinal(serialVersionUid.getAccessFlags()));

        assertTrue(Arrays.stream(resultClass.getMethods())
                .anyMatch(method -> method.getName().equals("lastIndexOf")));
    }

    private static void assertClosed(Executable operation) {
        IllegalStateException exception = assertThrows(IllegalStateException.class, operation);
        assertEquals("Class index is closed", exception.getMessage());
    }

    private static byte[] readClassBytes(Class<?> type) throws Exception {
        try (InputStream input = type.getResourceAsStream(type.getSimpleName() + ".class")) {
            assertNotNull(input);
            return input.readAllBytes();
        }
    }

    private static byte[] compileFixture(Path workspace, String sourceText) throws Exception {
        Path source = workspace.resolve("src/mixed/Fixture.java");
        Path classes = workspace.resolve("classes");
        Files.createDirectories(source.getParent());
        Files.createDirectories(classes);
        Files.writeString(source, sourceText);

        var compiler = ToolProvider.getSystemJavaCompiler();
        assertNotNull(compiler);
        assertEquals(0, compiler.run(null, null, null, "-d", classes.toString(), source.toString()));
        return Files.readAllBytes(classes.resolve("mixed/Fixture.class"));
    }

    private static void deleteTree(Path root) throws Exception {
        if (!Files.exists(root)) {
            return;
        }
        try (Stream<Path> paths = Files.walk(root)) {
            for (Path path : paths.sorted(Comparator.reverseOrder()).toList()) {
                Files.deleteIfExists(path);
            }
        }
    }

    private static void await(CountDownLatch latch) {
        try {
            if (!latch.await(5, TimeUnit.SECONDS)) {
                throw new AssertionError("Timed out waiting to release the in-flight index operation");
            }
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new AssertionError("Interrupted while waiting to release the in-flight index operation", e);
        }
    }
}
