package com.github.tth05.jindex;

import org.junit.jupiter.api.AfterAll;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestInstance;

import java.lang.reflect.Modifier;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.Arrays;
import java.util.Collections;

import static org.junit.jupiter.api.Assertions.*;

@TestInstance(TestInstance.Lifecycle.PER_CLASS)
public class BasicTest {

    private ClassIndex index;

    @BeforeAll
    public void init() {
        SampleClassesHelper.createSamplesJar();
        ClassIndex tempIndex = ClassIndex.fromJars(Collections.singletonList("src/test/resources/Samples.jar"));
        tempIndex.saveToFile("index");
        this.index = ClassIndex.fromFile("index");

        System.out.println(tempIndex.getBuildTimeInfo().toFormattedString());
    }

    @AfterAll
    public void cleanup() {
        assertDoesNotThrow(() -> Files.deleteIfExists(Paths.get("index")));
    }

    @Test
    public void testFindClass() {
        IndexedClass singleClass = index.findClass("java/lang", "String");
        assertNotNull(singleClass);
        assertEquals("java/lang/String", singleClass.getNameWithPackage());
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
}
