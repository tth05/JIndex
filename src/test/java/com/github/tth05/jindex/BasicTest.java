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
        IndexedClass resultClass = results[0];
        assertEquals(1, resultClass.getFields().length);
        assertEquals(10, resultClass.getMethods().length);
        assertEquals("com.github.tth05.jindex.ClassIndex", resultClass.getNameWithPackage());
        assertEquals("pointer", resultClass.getFields()[0].getName());
        assertTrue(Modifier.isPrivate(resultClass.getFields()[0].getAccessFlags()));
        assertEquals("findClasses", resultClass.getMethods()[2].getName());
        assertTrue(Modifier.isPublic(resultClass.getAccessFlags()));
        assertTrue(Modifier.isNative(resultClass.getMethods()[2].getAccessFlags()));
        assertTrue(Modifier.isPublic(resultClass.getMethods()[2].getAccessFlags()));
    }
}
