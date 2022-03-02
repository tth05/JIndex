package com.github.tth05.jindex;

public class IndexedSignature {

    private final long pointer;
    private final long classIndexPointer;

    public IndexedSignature(long classIndexPointer, long pointer) {
        this.classIndexPointer = classIndexPointer;
        this.pointer = pointer;
    }

    public native boolean isUnresolved();

    public native boolean isVoid();

    public native boolean isPrimitive();

    public native boolean isArray();

    /**
     * @return An indexed class representing the type of signature, or {@code null} if {@link #isUnresolved()},
     * {@link #isVoid()}, {@link #isPrimitive()} or {@link #isArray()} return {@code true}.
     */
    public native IndexedClass getType();

    /**
     * @return The class of the primitive type which is represented by this signature, or {@code null} if
     * {@link #isPrimitive()} returns {@code false}.
     */
    public native Class<?> getPrimitiveType();

    /**
     * @return The inner component of this array, or {@code null} if {@link #isArray()} returns {@code false}.
     */
    public native IndexedSignature getArrayComponent();
}
