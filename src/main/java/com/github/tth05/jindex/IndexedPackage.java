package com.github.tth05.jindex;

public class IndexedPackage {

    private final long classIndexPointer;
    private final long pointer;

    public IndexedPackage(long classIndexPointer, long pointer) {
        this.classIndexPointer = classIndexPointer;
        this.pointer = pointer;
    }

    public native String getName();
    public native String getNameWithParents();
    public native String getNameWithParentsDot();

    public native IndexedClass[] getClasses();
    public native IndexedPackage[] getSubPackages();
}
