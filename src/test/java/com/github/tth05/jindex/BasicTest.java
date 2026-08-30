package com.github.tth05.jindex;

import org.junit.jupiter.api.AfterAll;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestInstance;
import org.junit.jupiter.api.function.Executable;

import java.io.InputStream;
import java.lang.reflect.Modifier;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Arrays;
import java.util.Collections;
import java.util.Comparator;
import java.util.EnumSet;
import java.util.List;
import java.util.Set;
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

        assertEquals("java/lang/String", index.findClass("java.lang.String").getNameWithPackage());
        assertEquals("java/util/Map$Entry", index.findClass("java/util/Map$Entry").getNameWithPackage());
        assertNull(index.findClass("java.lang.DoesNotExist"));
    }

    @Test
    public void testFindClassesWithPackageQualifiedQuery() {
        IndexedClass[] exactPackage = index.findClasses(
                "java.lang.Str",
                SearchOptions.with(
                        SearchOptions.SearchMode.PREFIX,
                        SearchOptions.MatchMode.IGNORE_CASE,
                        10
                )
        );
        assertTrue(Arrays.stream(exactPackage)
                .anyMatch(indexedClass -> indexedClass.getNameWithPackage().equals("java/lang/String")));
        assertTrue(Arrays.stream(exactPackage)
                .allMatch(indexedClass -> indexedClass.getNameWithPackage().startsWith("java/lang/")));

        IndexedClass[] otherPackage = index.findClasses(
                "java.util.String",
                SearchOptions.with(
                        SearchOptions.SearchMode.CONTAINS,
                        SearchOptions.MatchMode.IGNORE_CASE,
                        10
                )
        );
        assertTrue(otherPackage.length > 0);
        assertTrue(Arrays.stream(otherPackage)
                .allMatch(indexedClass -> indexedClass.getNameWithPackage().startsWith("java/util/")));
    }

    @Test
    public void testFindClassesByCompleteBinaryName() {
        SearchOptions contains = SearchOptions.with(
                SearchOptions.SearchMode.CONTAINS,
                SearchOptions.MatchMode.IGNORE_CASE,
                10
        );

        IndexedClass[] partialPackage = index.findClassesByBinaryName("lang.str", contains);
        assertTrue(Arrays.stream(partialPackage)
                .anyMatch(indexedClass -> indexedClass.getNameWithPackage().equals("java/lang/String")));

        IndexedClass[] slashSeparated = index.findClassesByBinaryName("java/util/map$entry", contains);
        assertTrue(Arrays.stream(slashSeparated)
                .anyMatch(indexedClass -> indexedClass.getNameWithPackage().equals("java/util/Map$Entry")));

        IndexedClass[] limited = index.findClassesByBinaryName("java/lang/", SearchOptions.with(
                SearchOptions.SearchMode.CONTAINS,
                SearchOptions.MatchMode.IGNORE_CASE,
                1
        ));
        assertEquals(1, limited.length);
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
                    "package mixed; public class Fixture {"
                            + " public int directField;"
                            + " public String directMethod(int value) { return Integer.toString(value); }"
                            + " }"
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
                assertEquals(1, fixture.getSourceId());
                assertTrue(Arrays.stream(fixture.getFields()).anyMatch(field -> field.getName().equals("directField")));
                assertFalse(Arrays.stream(fixture.getFields()).anyMatch(field -> field.getName().equals("archiveField")));

                SymbolSearchResult[] symbols = mixedIndex.findSymbols(
                        "direct",
                        SearchOptions.with(
                                SearchOptions.SearchMode.PREFIX,
                                SearchOptions.MatchMode.IGNORE_CASE,
                                10
                        ),
                        EnumSet.of(SymbolKind.FIELD, SymbolKind.METHOD)
                );
                assertArrayEquals(
                        new String[]{"directField:I", "directMethod:(I)Ljava/lang/String;"},
                        Arrays.stream(symbols)
                                .map(symbol -> symbol.name() + ":" + symbol.descriptor())
                                .toArray(String[]::new)
                );
                assertTrue(Arrays.stream(symbols).allMatch(symbol -> symbol.sourceId() == 1));
                assertTrue(Arrays.stream(symbols).allMatch(symbol -> symbol.ownerInternalName().equals("mixed/Fixture")));
                assertEquals(2, Arrays.stream(symbols).mapToLong(SymbolSearchResult::symbolId).distinct().count());
                assertArrayEquals(
                        new String[]{"Fixture"},
                        Arrays.stream(mixedIndex.findClasses("Fixture", SearchOptions.defaultOptions(), 1))
                                .map(IndexedClass::getName)
                                .toArray(String[]::new)
                );
                assertEquals(0, mixedIndex.findClasses("Fixture", SearchOptions.defaultOptions(), 0).length);
                assertArrayEquals(
                        new String[]{"mixed/Fixture"},
                        Arrays.stream(mixedIndex.findClassesByBinaryName(
                                        "mixed.fixture",
                                        SearchOptions.defaultOptions(),
                                        1
                                ))
                                .map(IndexedClass::getNameWithPackage)
                                .toArray(String[]::new)
                );
                assertEquals(0, mixedIndex.findClassesByBinaryName(
                        "mixed.fixture",
                        SearchOptions.defaultOptions(),
                        0
                ).length);
                assertEquals(0, mixedIndex.findClassesByBinaryName(
                        "mixed.fixture",
                        SearchOptions.defaultOptions(),
                        new int[0]
                ).length);
                assertThrows(IllegalArgumentException.class, () -> mixedIndex.findClassesByBinaryName(
                        "mixed.fixture",
                        SearchOptions.defaultOptions(),
                        -1
                ));
                assertEquals(2, mixedIndex.findSymbols(
                        "direct",
                        SearchOptions.defaultOptions(),
                        EnumSet.of(SymbolKind.FIELD, SymbolKind.METHOD),
                        1
                ).length);
                assertEquals(0, mixedIndex.findSymbols(
                        "direct",
                        SearchOptions.defaultOptions(),
                        EnumSet.of(SymbolKind.FIELD, SymbolKind.METHOD),
                        0
                ).length);
                assertEquals(0, mixedIndex.findSymbols(
                        "direct",
                        SearchOptions.defaultOptions(),
                        EnumSet.of(SymbolKind.FIELD, SymbolKind.METHOD),
                        new int[0]
                ).length);
            }

            try (ClassIndex explicitIndex = ClassIndex.fromSources(List.of(
                    IndexSource.classFile(42, directClass),
                    IndexSource.classFile(42, readClassBytes(java.util.ArrayList.class)),
                    IndexSource.archive(7, archive.toString())
            ))) {
                IndexedClass fixture = explicitIndex.findClass("mixed", "Fixture");
                IndexedClass arrayList = explicitIndex.findClass("java/util", "ArrayList");
                assertNotNull(fixture);
                assertNotNull(arrayList);
                assertEquals(42, fixture.getSourceId());
                assertEquals(42, arrayList.getSourceId());
                assertTrue(Arrays.stream(fixture.getFields())
                        .anyMatch(field -> field.getName().equals("directField")));
            }

            try (ClassIndex archiveFirst = ClassIndex.fromSources(List.of(
                    IndexSource.archive(7, archive.toString()),
                    IndexSource.classFile(42, directClass)
            ))) {
                IndexedClass fixture = archiveFirst.findClass("mixed", "Fixture");
                assertNotNull(fixture);
                assertEquals(7, fixture.getSourceId());
                assertTrue(Arrays.stream(fixture.getFields())
                        .anyMatch(field -> field.getName().equals("archiveField")));
            }
        } finally {
            deleteTree(workspace);
        }
    }

    @Test
    public void testSymbolSearchRanksAcrossKindsBeforeApplyingLimit() throws Exception {
        Path workspace = Files.createTempDirectory("jindex-symbol-order-");
        try {
            byte[] fixture = compileFixture(
                    workspace.resolve("compile"),
                    "package mixed; public class Fixture {"
                            + " public int matchZ;"
                            + " public void matchA() {}"
                            + " }"
            );
            try (ClassIndex fixtureIndex = ClassIndex.fromBytes(List.of(fixture))) {
                SymbolSearchResult[] symbols = fixtureIndex.findSymbols(
                        "match",
                        SearchOptions.with(
                                SearchOptions.SearchMode.PREFIX,
                                SearchOptions.MatchMode.IGNORE_CASE,
                                1
                        ),
                        EnumSet.of(SymbolKind.FIELD, SymbolKind.METHOD)
                );
                assertEquals(1, symbols.length);
                assertEquals(SymbolKind.METHOD, symbols[0].kind());
                assertEquals("matchA", symbols[0].name());

                SymbolSearchResult[] contains = fixtureIndex.findSymbols(
                        "atch",
                        SearchOptions.with(
                                SearchOptions.SearchMode.CONTAINS,
                                SearchOptions.MatchMode.IGNORE_CASE,
                                10
                        ),
                        EnumSet.of(SymbolKind.FIELD, SymbolKind.METHOD)
                );
                assertArrayEquals(
                        new String[]{"matchA", "matchZ"},
                        Arrays.stream(contains).map(SymbolSearchResult::name).toArray(String[]::new)
                );
            }
        } finally {
            deleteTree(workspace);
        }
    }

    @Test
    public void testJava21ClassFileStructuresAreIndexed() throws Exception {
        Path workspace = Files.createTempDirectory("jindex-java21-classfile-");
        try {
            byte[] record = compileFixture(
                    workspace.resolve("record"),
                    "package mixed; public record Fixture(int value) {"
                            + " public String display() { return \"value=\" + value; }"
                            + " }"
            );
            byte[] sealedClass = compileFixture(
                    workspace.resolve("sealed"),
                    "package mixed; public sealed class Fixture permits Fixture.Child {"
                            + " public static final class Child extends Fixture {}"
                            + " }"
            );

            try (ClassIndex recordIndex = ClassIndex.fromBytes(List.of(record));
                 ClassIndex sealedIndex = ClassIndex.fromBytes(List.of(sealedClass))) {
                IndexedClass indexedRecord = recordIndex.findClass("mixed", "Fixture");
                assertNotNull(indexedRecord);
                assertTrue(Arrays.stream(indexedRecord.getFields())
                        .anyMatch(field -> field.getName().equals("value")));
                assertTrue(Arrays.stream(indexedRecord.getMethods())
                        .anyMatch(method -> method.getName().equals("display")));
                assertNotNull(sealedIndex.findClass("mixed", "Fixture"));
            }
        } finally {
            deleteTree(workspace);
        }
    }

    @Test
    public void testNonAsciiMemberDoesNotExcludeItsClass() throws Exception {
        Path workspace = Files.createTempDirectory("jindex-unicode-member-");
        try {
            byte[] fixture = compileFixture(
                    workspace.resolve("compile"),
                    "package mixed; public class Fixture {"
                            + " public int supportedField;"
                            + " public void supportedMethod() {}"
                            + " public void generateGrötzschGraph() {}"
                            + " }"
            );
            try (ClassIndex fixtureIndex = ClassIndex.fromBytes(List.of(fixture))) {
                IndexedClass indexedClass = fixtureIndex.findClass("mixed", "Fixture");
                assertNotNull(indexedClass);
                assertTrue(Arrays.stream(indexedClass.getFields())
                        .anyMatch(field -> field.getName().equals("supportedField")));
                assertTrue(Arrays.stream(indexedClass.getMethods())
                        .anyMatch(method -> method.getName().equals("supportedMethod")));
                assertFalse(Arrays.stream(indexedClass.getMethods())
                        .anyMatch(method -> method.getName().equals("generateGrötzschGraph")));
            }
        } finally {
            deleteTree(workspace);
        }
    }

    @Test
    public void testMultiReleaseArchiveUsesTargetRuntimeView() throws Exception {
        Path workspace = Files.createTempDirectory("jindex-multi-release-");
        try {
            byte[] baseClass = compileFixture(
                    workspace.resolve("base"),
                    "package mixed; public class Fixture { public int baseField; }"
            );
            byte[] java21Class = compileFixture(
                    workspace.resolve("java21"),
                    "package mixed; public class Fixture { public int java21Field; }"
            );
            Path multiRelease = workspace.resolve("multi-release.jar");
            try (ZipOutputStream output = new ZipOutputStream(Files.newOutputStream(multiRelease))) {
                output.putNextEntry(new ZipEntry("META-INF/MANIFEST.MF"));
                output.write(("Manifest-Version: 1.0\r\n"
                        + "Multi-Release: true\r\n\r\n").getBytes(StandardCharsets.UTF_8));
                output.closeEntry();
                writeClassEntry(output, "mixed/Fixture.class", baseClass);
                writeClassEntry(output, "META-INF/versions/21/mixed/Fixture.class", java21Class);
            }

            Path ordinaryArchive = workspace.resolve("ordinary.jar");
            try (ZipOutputStream output = new ZipOutputStream(Files.newOutputStream(ordinaryArchive))) {
                writeClassEntry(output, "mixed/Fixture.class", baseClass);
                writeClassEntry(output, "META-INF/versions/21/mixed/Fixture.class", java21Class);
            }

            try (ClassIndex java17View = ClassIndex.fromSources(
                    List.of(IndexSource.archive(1, multiRelease.toString())),
                    new IndexBuildOptions(17)
            ); ClassIndex java21View = ClassIndex.fromSources(
                    List.of(IndexSource.archive(1, multiRelease.toString())),
                    new IndexBuildOptions(21)
            ); ClassIndex ordinaryView = ClassIndex.fromSources(
                    List.of(IndexSource.archive(1, ordinaryArchive.toString())),
                    new IndexBuildOptions(21)
            )) {
                assertFieldSet(java17View.findClass("mixed", "Fixture"), "baseField");
                assertFieldSet(java21View.findClass("mixed", "Fixture"), "java21Field");
                assertFieldSet(ordinaryView.findClass("mixed", "Fixture"), "baseField");
            }
        } finally {
            deleteTree(workspace);
        }
    }

    @Test
    public void testReferencesAreResolvedAndPersisted() throws Exception {
        Path workspace = Files.createTempDirectory("jindex-references-");
        Path snapshot = workspace.resolve("index.zip");
        try {
            List<byte[]> classes = compileFixtureClasses(
                    workspace.resolve("compile"),
                    "package mixed;"
                            + " public class Fixture {"
                            + "   Target field;"
                            + "   void caller(Target target) {"
                            + "     target.value++;"
                            + "     target.run();"
                            + "     target.run();"
                            + "   }"
                            + "   Runnable methodReference(Target target) { return target::run; }"
                            + " }"
                            + " class Target { int value; void run() {} }",
                    "mixed/Fixture.class",
                    "mixed/Target.class"
            );

            try (ClassIndex fixtureIndex = ClassIndex.fromBytes(classes)) {
                assertReferenceGraph(fixtureIndex);
                fixtureIndex.saveToFile(snapshot.toString());
            }
            try (ClassIndex persistedIndex = ClassIndex.fromFile(snapshot.toString())) {
                assertReferenceGraph(persistedIndex);
            }
        } finally {
            deleteTree(workspace);
        }
    }

    @Test
    public void testClassReferenceKindsDescribeHierarchyMetadataAndRuntimeUse() throws Exception {
        Path workspace = Files.createTempDirectory("jindex-reference-kinds-");
        try {
            List<byte[]> classes = compileFixtureClasses(
                    workspace,
                    "package mixed;"
                            + " @Marker(Target.class)"
                            + " public class Fixture extends Target {"
                            + "   Target make(Object value) {"
                            + "     Target created = new Target();"
                            + "     return value instanceof Target ? (Target) value : created;"
                            + "   }"
                            + " }"
                            + " class Target {}"
                            + " @interface Marker { Class<?> value(); }",
                    "mixed/Fixture.class",
                    "mixed/Target.class",
                    "mixed/Marker.class"
            );
            try (ClassIndex fixtureIndex = ClassIndex.fromBytes(classes)) {
                ReferenceResult[] references = fixtureIndex.findReferences(
                        ReferenceTarget.classTarget("mixed/Target"),
                        100
                ).results();
                ReferenceResult classSite = Arrays.stream(references)
                        .filter(reference -> reference.ownerInternalName().equals("mixed/Fixture"))
                        .filter(reference -> reference.kind() == ReferenceSiteKind.CLASS)
                        .findFirst()
                        .orElseThrow();
                assertEquals(
                        Set.of(
                                ReferenceKind.CLASS_HIERARCHY,
                                ReferenceKind.CLASS_ANNOTATION_OR_METADATA
                        ),
                        classSite.kinds()
                );
                ReferenceResult methodSite = Arrays.stream(references)
                        .filter(reference -> reference.name().equals("make"))
                        .findFirst()
                        .orElseThrow();
                assertTrue(methodSite.kinds().containsAll(Set.of(
                        ReferenceKind.CLASS_DECLARATION,
                        ReferenceKind.CLASS_RUNTIME_TYPE,
                        ReferenceKind.CLASS_MEMBER_USAGE
                )));
            }
        } finally {
            deleteTree(workspace);
        }
    }

    @Test
    public void testSemanticStringLiteralsAreExactAndPersisted() throws Exception {
        Path workspace = Files.createTempDirectory("jindex-literals-");
        Path snapshot = workspace.resolve("index.zip");
        try {
            byte[] fixture = compileFixture(
                    workspace.resolve("compile"),
                    "package mixed;"
                            + " @Marker(\"annotation-value\")"
                            + " public class Fixture {"
                            + "   static final String CONSTANT = \"constant-value\";"
                            + "   String unicode() { return \"snowman ☃\"; }"
                            + "   String embeddedNull() { return \"embedded\\0null\"; }"
                            + "   String loneSurrogate() { return \"\\uD800\"; }"
                            + "   String concat(String value) { return \"recipe-prefix=\" + value; }"
                            + " }"
                            + " @interface Marker { String value(); }"
            );

            try (ClassIndex fixtureIndex = ClassIndex.fromBytes(List.of(fixture))) {
                assertLiteralIndex(fixtureIndex);
                fixtureIndex.saveToFile(snapshot.toString());
            }
            try (ClassIndex persistedIndex = ClassIndex.fromFile(snapshot.toString())) {
                assertLiteralIndex(persistedIndex);
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

    @Test
    public void testSymbolSearchSurvivesPersistence() {
        IndexStatistics statistics = index.getStatistics();
        assertTrue(statistics.classCount() > 0);
        assertTrue(statistics.fieldCount() > 0);
        assertTrue(statistics.methodCount() > 0);
        assertTrue(statistics.referenceSiteCount() > 0);
        SymbolSearchResult[] results = index.findSymbols(
                "lastIndexOf",
                SearchOptions.with(
                        SearchOptions.SearchMode.PREFIX,
                        SearchOptions.MatchMode.MATCH_CASE,
                        100
                ),
                EnumSet.of(SymbolKind.METHOD)
        );

        assertTrue(Arrays.stream(results).anyMatch(result ->
                result.kind() == SymbolKind.METHOD
                        && result.ownerInternalName().equals("java/lang/String")
                        && result.name().equals("lastIndexOf")
                        && result.descriptor().equals("(I)I")
        ));
        assertEquals(0, index.findClass("java/lang", "String").getSourceId());
        assertEquals(
                0,
                index.findSymbols(
                        "lastIndexOf",
                        SearchOptions.defaultOptions(),
                        EnumSet.noneOf(SymbolKind.class)
                ).length
        );
    }

    @Test
    public void testLegacySnapshotCompressionIsRejected() throws Exception {
        Path legacySnapshot = Files.createTempFile("jindex-legacy-", ".zip");
        try {
            writeIndexPayload(
                    legacySnapshot,
                    new byte[]{'J', 'I', 'N', 'D', 'E', 'X', 0, 0, 2, 0}
            );
            ClassIndexBuildingException unsupportedCompression = assertThrows(
                    ClassIndexBuildingException.class,
                    () -> ClassIndex.fromFile(legacySnapshot.toString())
            );
            assertTrue(unsupportedCompression.getMessage().contains(
                    "Unsupported JIndex snapshot compression Deflated; expected Zstandard"
            ));
        } finally {
            Files.deleteIfExists(legacySnapshot);
        }
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

    private static List<byte[]> compileFixtureClasses(
            Path workspace,
            String sourceText,
            String... relativeClassPaths
    ) throws Exception {
        Path source = workspace.resolve("src/mixed/Fixture.java");
        Path classes = workspace.resolve("classes");
        Files.createDirectories(source.getParent());
        Files.createDirectories(classes);
        Files.writeString(source, sourceText);

        var compiler = ToolProvider.getSystemJavaCompiler();
        assertNotNull(compiler);
        assertEquals(0, compiler.run(null, null, null, "-d", classes.toString(), source.toString()));
        return Arrays.stream(relativeClassPaths)
                .map(classes::resolve)
                .map(path -> assertDoesNotThrow(() -> Files.readAllBytes(path)))
                .toList();
    }

    private static void assertReferenceGraph(ClassIndex fixtureIndex) {
        ReferenceResult[] classReferences = fixtureIndex.findReferences(
                ReferenceTarget.classTarget("mixed/Target"),
                100
        ).results();
        assertTrue(Arrays.stream(classReferences).anyMatch(reference ->
                reference.kind() == ReferenceSiteKind.FIELD
                        && reference.ownerInternalName().equals("mixed/Fixture")
                        && reference.name().equals("field")
                        && reference.descriptor().equals("Lmixed/Target;")
                        && reference.kinds().equals(Set.of(ReferenceKind.CLASS_DECLARATION))
        ));
        assertTrue(Arrays.stream(classReferences).anyMatch(reference ->
                reference.kind() == ReferenceSiteKind.METHOD
                        && reference.ownerInternalName().equals("mixed/Fixture")
                        && reference.name().equals("caller")
                        && reference.kinds().containsAll(Set.of(
                                ReferenceKind.CLASS_DECLARATION,
                                ReferenceKind.CLASS_MEMBER_USAGE
                        ))
        ));

        ReferenceResult fieldReference = Arrays.stream(fixtureIndex.findReferences(
                        ReferenceTarget.fieldTarget("mixed/Target", "value", "I"),
                        100
                ).results())
                .filter(reference -> reference.name().equals("caller"))
                .findFirst()
                .orElseThrow();
        assertEquals(ReferenceSiteKind.METHOD, fieldReference.kind());
        assertEquals(2, fieldReference.occurrenceCount());
        assertEquals(
                Set.of(ReferenceKind.FIELD_READ, ReferenceKind.FIELD_WRITE),
                fieldReference.kinds()
        );

        ReferenceTarget methodTarget = ReferenceTarget.methodTarget("mixed/Target", "run", "()V");
        ReferenceResult[] methodReferences = fixtureIndex.findReferences(methodTarget, 100).results();
        ReferenceResult directCalls = Arrays.stream(methodReferences)
                .filter(reference -> reference.name().equals("caller"))
                .findFirst()
                .orElseThrow();
        assertEquals(2, directCalls.occurrenceCount());
        assertEquals(Set.of(ReferenceKind.METHOD_INVOKE), directCalls.kinds());
        ReferenceResult methodReference = Arrays.stream(methodReferences)
                .filter(reference -> reference.name().equals("methodReference"))
                .findFirst()
                .orElseThrow();
        assertEquals(1, methodReference.occurrenceCount());
        assertEquals(Set.of(ReferenceKind.METHOD_HANDLE), methodReference.kinds());

        ReferenceSearchPage limited = fixtureIndex.findReferences(methodTarget, 1);
        assertEquals(1, limited.results().length);
        assertTrue(limited.truncated());
        ReferenceSearchPage fixtureSource = fixtureIndex.findReferences(methodTarget, 100, 0, 0);
        assertEquals(methodReferences.length, fixtureSource.results().length);
        assertFalse(fixtureSource.truncated());
        assertEquals(0, fixtureIndex.findReferences(methodTarget, 100, 1).results().length);
        assertEquals(0, fixtureIndex.findReferences(methodTarget, 100, new int[0]).results().length);
        assertThrows(IllegalArgumentException.class, () -> fixtureIndex.findReferences(methodTarget, 0));
        assertThrows(IllegalArgumentException.class, () -> fixtureIndex.findReferences(methodTarget, 1, -1));

        assertTrue(fixtureIndex.getStatistics().referenceSiteCount() >= 5);
    }

    private static void assertLiteralIndex(ClassIndex fixtureIndex) {
        assertLiteralSite(fixtureIndex, "annotation-value", ReferenceSiteKind.CLASS, "");
        assertLiteralSite(fixtureIndex, "constant-value", ReferenceSiteKind.FIELD, "CONSTANT");
        assertLiteralSite(fixtureIndex, "snowman ☃", ReferenceSiteKind.METHOD, "unicode");
        assertLiteralSite(fixtureIndex, "embedded\0null", ReferenceSiteKind.METHOD, "embeddedNull");
        assertLiteralSite(
                fixtureIndex,
                new String(new char[]{'\uD800'}),
                ReferenceSiteKind.METHOD,
                "loneSurrogate"
        );
        assertEquals(0, fixtureIndex.findLiteralReferences("recipe-prefix=", 10).results().length);
        LiteralSearchPage values = fixtureIndex.findLiteralsContaining("value", 10);
        assertArrayEquals(
                new String[]{"annotation-value", "constant-value"},
                Arrays.stream(values.results()).map(LiteralSearchResult::value).toArray(String[]::new)
        );
        assertTrue(Arrays.stream(values.results()).allMatch(result -> Arrays.equals(new int[]{0}, result.sourceIds())));
        assertFalse(values.truncated());
        LiteralSearchPage limited = fixtureIndex.findLiteralsContaining("value", 1);
        assertArrayEquals(
                new String[]{"annotation-value"},
                Arrays.stream(limited.results()).map(LiteralSearchResult::value).toArray(String[]::new)
        );
        assertTrue(limited.truncated());
        assertArrayEquals(
                new String[]{new String(new char[]{'\uD800'})},
                Arrays.stream(fixtureIndex.findLiteralsContaining(new String(new char[]{'\uD800'}), 10).results())
                        .map(LiteralSearchResult::value)
                        .toArray(String[]::new)
        );
        assertEquals(2, fixtureIndex.findLiteralsContaining("value", 10, 0).results().length);
        assertEquals(0, fixtureIndex.findLiteralsContaining("value", 10, 1).results().length);
        assertEquals(0, fixtureIndex.findLiteralsContaining("value", 10, new int[0]).results().length);
        assertEquals(5, fixtureIndex.getStatistics().literalCount());
        assertEquals(5, fixtureIndex.getStatistics().literalOccurrenceCount());
    }

    private static void assertLiteralSite(
            ClassIndex fixtureIndex,
            String literal,
            ReferenceSiteKind kind,
            String name
    ) {
        ReferenceResult[] references = fixtureIndex.findLiteralReferences(literal, 10).results();
        assertEquals(1, references.length);
        assertEquals(kind, references[0].kind());
        assertEquals("mixed/Fixture", references[0].ownerInternalName());
        assertEquals(name, references[0].name());
        assertEquals(1, references[0].occurrenceCount());
        assertEquals(Set.of(ReferenceKind.STRING_LITERAL), references[0].kinds());
        assertEquals(1, fixtureIndex.findLiteralReferences(literal, 10, 0).results().length);
        assertEquals(0, fixtureIndex.findLiteralReferences(literal, 10, 1).results().length);
    }

    private static void writeIndexPayload(Path output, byte[] payload) throws Exception {
        try (ZipOutputStream zip = new ZipOutputStream(Files.newOutputStream(output))) {
            zip.putNextEntry(new ZipEntry("index"));
            zip.write(payload);
            zip.closeEntry();
        }
    }

    private static void writeClassEntry(ZipOutputStream output, String name, byte[] bytes) throws Exception {
        output.putNextEntry(new ZipEntry(name));
        output.write(bytes);
        output.closeEntry();
    }

    private static void assertFieldSet(IndexedClass indexedClass, String expectedName) {
        assertNotNull(indexedClass);
        assertArrayEquals(
                new String[]{expectedName},
                Arrays.stream(indexedClass.getFields()).map(IndexedField::getName).toArray(String[]::new)
        );
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
