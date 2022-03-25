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

    public native String getPackage();

    public native String getNameWithPackage();

    public String getNameWithPackageDot() {
        return getNameWithPackage().replace('/', '.');
    }

    public native IndexedClass getSuperClass();
    public native IndexedClass[] getInterfaces();

    public native String getGenericSignatureString();

    public native IndexedField[] getFields();

    public native IndexedMethod[] getMethods();

    public native short getAccessFlags();

    @Override
    public String toString() {
        return "FindClassesResult{" +
               "className='" + getNameWithPackage() + '\'' +
               ", methodNames=" + Arrays.toString(getMethods()) +
               '}';
    }
}
