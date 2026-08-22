package com.github.tth05.jindex;

abstract class ClassIndexChildObject {

    private volatile long classIndexPointer;
    private final long pointer;
    @SuppressWarnings("FieldCanBeLocal")
    private final ClassIndex owner;

    public ClassIndexChildObject(long classIndexPointer, long pointer) {
        this.classIndexPointer = classIndexPointer;
        this.pointer = pointer;
        this.owner = classIndexPointer == 0 ? null : ClassIndex.ownerFor(classIndexPointer);
    }

    final long classIndexPointer() {
        return this.classIndexPointer;
    }

    final void clearClassIndexPointer() {
        this.classIndexPointer = 0;
    }
}
