package com.github.tth05.jindex;

abstract class ClassChildObject extends ClassIndexChildObject {

    private final long classPointer;

    public ClassChildObject(long classIndexPointer, long classPointer, long pointer) {
        super(classIndexPointer, pointer);
        this.classPointer = classPointer;
    }
}
