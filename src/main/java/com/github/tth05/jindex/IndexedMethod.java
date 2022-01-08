package com.github.tth05.jindex;

public class IndexedMethod {

    private final long classIndexPointer;
    private final long pointer;

    public IndexedMethod(long classIndexPointer, long pointer) {
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
