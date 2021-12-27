package com.github.tth05.jindex;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

public class BasicTest {

    @Test
    public void test() {
        ClassIndex index = new ClassIndex("src/test/resources/classIndex.serialized");
        IndexedClass[] results = index.findClasses("MixinEvent", 500);

        assertEquals(1, results.length);
        assertEquals(1, results[0].getMethods().length);
        assertEquals("com.github.minecraft_ta.totalperformance.mixin.MixinEventBus", results[0].getNameWithPackage());
        assertEquals("register", results[0].getMethods()[0].getName());
    }
}
