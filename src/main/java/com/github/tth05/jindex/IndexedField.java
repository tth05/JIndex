package com.github.tth05.jindex;

public class IndexedField {

    private final long classIndexPointer;
    private final long classPointer;
    private final long pointer;

    public IndexedField(long classIndexPointer, long classPointer, long pointer) {
        this.classIndexPointer = classIndexPointer;
        this.classPointer = classPointer;
        this.pointer = pointer;
    }

    public native String getName();

    public native short getAccessFlags();

    public native String getDescriptorString();
    public native String getGenericSignatureString();

    @Override
    public String toString() {
        return getName();
    }
}
