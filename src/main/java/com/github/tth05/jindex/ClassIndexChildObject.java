package com.github.tth05.jindex;

import java.util.Objects;
import java.util.function.IntSupplier;
import java.util.function.Supplier;

abstract class ClassIndexChildObject {

    private volatile long classIndexPointer;
    private final long pointer;
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

    final <T> T executeWhileOwnerOpen(Supplier<T> operation) {
        return Objects.requireNonNull(this.owner, "Class index owner").executeWhileOpen(operation);
    }

    final int executeWhileOwnerOpen(IntSupplier operation) {
        return Objects.requireNonNull(this.owner, "Class index owner").executeWhileOpen(operation);
    }
}
