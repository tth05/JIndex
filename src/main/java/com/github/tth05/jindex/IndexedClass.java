package com.github.tth05.jindex;

import java.util.Arrays;

public class IndexedClass extends ClassIndexChildObject {

    public IndexedClass(long classIndexPointer, long pointer) {
        super(classIndexPointer, pointer);
    }

    public native String getName();

    public native IndexedPackage getPackage();

    public native String getNameWithPackage();
    public native String getNameWithPackageDot();

    public native String getGenericSignatureString();

    public native IndexedClass getEnclosingClass();
    public native String getEnclosingMethodNameAndDesc();
    public native IndexedClass getSuperClass();
    public native IndexedClass[] getInterfaces();

    public native IndexedField[] getFields();

    public native IndexedMethod[] getMethods();

    public native int getAccessFlags();

    @Override
    public String toString() {
        return "FindClassesResult{" +
               "className='" + getNameWithPackage() + '\'' +
               ", methodNames=" + Arrays.toString(getMethods()) +
               '}';
    }
}
