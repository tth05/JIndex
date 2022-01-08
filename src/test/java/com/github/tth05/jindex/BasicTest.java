package com.github.tth05.jindex;

import org.junit.jupiter.api.Test;

import java.lang.reflect.Modifier;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.List;
import java.util.stream.Collectors;

import static org.junit.jupiter.api.Assertions.*;

public class BasicTest {

    private List<byte[]> readTestClasses() {
        return assertDoesNotThrow(() ->
                Files.walk(Paths.get("./src/test/resources/"), 1)
                        .skip(1)
                        .map(p -> assertDoesNotThrow(() -> Files.readAllBytes(p))).collect(Collectors.toList())
        );
    }

    @Test
    public void test() {
        ClassIndex index = new ClassIndex(readTestClasses());
        IndexedClass[] results = index.findClasses("ClassIndex", 500);

        assertEquals(1, results.length);
        assertEquals(10, results[0].getMethods().length);
        assertEquals("com.github.tth05.jindex.ClassIndex", results[0].getNameWithPackage());
        assertEquals("findClasses", results[0].getMethods()[2].getName());
        assertTrue(Modifier.isPublic(results[0].getAccessFlags()));
        assertTrue(Modifier.isNative(results[0].getMethods()[2].getAccessFlags()));
        assertTrue(Modifier.isPublic(results[0].getMethods()[2].getAccessFlags()));
    }
}
