package com.github.tth05.jindex;

import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestInstance;

import java.lang.reflect.Modifier;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

@TestInstance(TestInstance.Lifecycle.PER_CLASS)
public class BasicTest {

    private ClassIndex index;

    @BeforeAll
    public void init() {
        List<byte[]> classes = SampleClassesHelper.loadSampleClasses();
        index = new ClassIndex(classes);
    }

    @Test
    public void testFindClass() {
        IndexedClass singleClass = index.findClass("com.sun.org.apache.xpath.internal.operations", "String");
        assertNotNull(singleClass);
        assertEquals("com.sun.org.apache.xpath.internal.operations.String", singleClass.getNameWithPackage());
    }

    @Test
    public void testFindClasses() {
        IndexedClass[] results = index.findClasses("String", 500);

        assertEquals(62, results.length);

        IndexedClass resultClass = results[0];
        assertEquals(1, resultClass.getFields().length);
        assertEquals(2, resultClass.getMethods().length);
        assertEquals("com.sun.org.apache.xpath.internal.operations.String", resultClass.getNameWithPackage());
        assertTrue(Modifier.isPublic(resultClass.getAccessFlags()));

        assertEquals("serialVersionUID", resultClass.getFields()[0].getName());
        assertTrue(Modifier.isStatic(resultClass.getFields()[0].getAccessFlags()));
        assertTrue(Modifier.isFinal(resultClass.getFields()[0].getAccessFlags()));

        assertEquals("operate", resultClass.getMethods()[1].getName());
        assertTrue(Modifier.isPublic(resultClass.getMethods()[1].getAccessFlags()));
    }
}
