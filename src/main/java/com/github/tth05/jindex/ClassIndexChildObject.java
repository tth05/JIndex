package com.github.tth05.jindex;

abstract class ClassIndexChildObject {

    private final long classIndexPointer;
    private final long pointer;

    public ClassIndexChildObject(long classIndexPointer, long pointer) {
        this.classIndexPointer = classIndexPointer;
        this.pointer = pointer;
    }
}
