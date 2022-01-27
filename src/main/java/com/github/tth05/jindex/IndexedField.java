package com.github.tth05.jindex;

public class IndexedField {

    private final long classIndexPointer;
    private final long pointer;

    public IndexedField(long classIndexPointer, long pointer) {
        this.classIndexPointer = classIndexPointer;
        this.pointer = pointer;
    }

    public native String getName();

    public native short getAccessFlags();

    @Override
    public String toString() {
        return getName();
    }
}
