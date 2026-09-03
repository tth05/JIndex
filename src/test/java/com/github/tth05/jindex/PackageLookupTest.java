package com.github.tth05.jindex;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import javax.tools.ToolProvider;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

final class PackageLookupTest {
    @TempDir Path temporaryDirectory;

    @Test
    void equalClassNamesUseForwardPackageOrderBeforeAndAfterPersistence() throws Exception {
        List<String> packages = List.of("", "aa", "aa/ba", "az", "ba", "ba/aa");
        var bytes = new ArrayList<byte[]>();
        var arguments = new ArrayList<String>(List.of("-d", temporaryDirectory.toString()));
        for (String name : packages.reversed()) {
            Path source = temporaryDirectory.resolve("source").resolve(name).resolve("Shared.java");
            Files.createDirectories(source.getParent());
            Files.writeString(source, (name.isEmpty() ? "" : "package " + name.replace('/', '.') + ";")
                    + "public class Shared {}");
            arguments.add(source.toString());
        }
        assertEquals(0, ToolProvider.getSystemJavaCompiler().run(null, null, null, arguments.toArray(String[]::new)));
        for (String name : packages.reversed()) {
            bytes.add(Files.readAllBytes(temporaryDirectory.resolve(name).resolve("Shared.class")));
        }
        Path snapshot = temporaryDirectory.resolve("index.zip");
        try (ClassIndex index = ClassIndex.fromBytes(bytes)) {
            assertLookups(index, packages);
            index.saveToFile(snapshot.toString());
        }
        try (ClassIndex index = ClassIndex.fromFile(snapshot.toString())) {
            assertLookups(index, packages);
        }
    }

    private static void assertLookups(ClassIndex index, List<String> packages) {
        assertEquals(packages, Arrays.stream(index.findClasses("Shared", SearchOptions.defaultOptions()).results())
                .map(type -> type.getPackage().getNameWithParents()).toList());
        for (String name : packages) {
            IndexedClass type = assertNotNullClass(index.findClass(name, "Shared"));
            assertEquals(name, type.getPackage().getNameWithParents());
            assertEquals(name.replace('/', '.'), type.getPackage().getNameWithParentsDot());
            String binaryName = name.isEmpty() ? "Shared" : name + "/Shared";
            assertEquals(binaryName, type.getNameWithPackage());
            assertTrue(Arrays.stream(index.findClassesByBinaryName(binaryName, SearchOptions.defaultOptions()).results())
                    .anyMatch(match -> match.getNameWithPackage().equals(binaryName)));
        }
        assertNull(index.findClass("a", "Shared"));
        assertNull(index.findClass("ba/a", "Shared"));
        assertNull(index.findClass("az/missing", "Shared"));
    }

    private static IndexedClass assertNotNullClass(IndexedClass type) {
        assertNotNull(type);
        return type;
    }
}
