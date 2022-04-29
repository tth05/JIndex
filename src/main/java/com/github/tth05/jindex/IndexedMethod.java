package com.github.tth05.jindex;

public class IndexedMethod extends ClassChildObject {

    public IndexedMethod(long classIndexPointer, long classPointer, long pointer) {
        super(classIndexPointer, classPointer, pointer);
    }

    public native IndexedClass getDeclaringClass();

    public native String getName();

    public native int getAccessFlags();

    public native String getDescriptorString();

    public native String getGenericSignatureString();

    public native IndexedMethod[] findImplementations(boolean includeBaseMethod);

    @Override
    public String toString() {
        return getName();
    }
}
