package com.github.tth05.jindex;

import java.util.Arrays;

public class IndexedClass {

    private final long classIndexPointer;
    private final long pointer;

    public IndexedClass(long classIndexPointer, long pointer) {
        this.classIndexPointer = classIndexPointer;
        this.pointer = pointer;
    }

    public native String getName();

    public native String getNameWithPackage();

    public native IndexedMethod[] getMethods();

    @Override
    public String toString() {
        return "FindClassesResult{" +
               "className='" + getNameWithPackage() + '\'' +
               ", methodNames=" + Arrays.toString(getMethods()) +
               '}';
    }
}
