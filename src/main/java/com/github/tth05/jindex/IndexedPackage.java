package com.github.tth05.jindex;

public class IndexedPackage extends ClassIndexChildObject{

    public IndexedPackage(long classIndexPointer, long pointer) {
        super(classIndexPointer, pointer);
    }

    public native String getName();
    public native String getNameWithParents();
    public native String getNameWithParentsDot();

    public native IndexedClass[] getClasses();
    public native IndexedPackage[] getSubPackages();
}
