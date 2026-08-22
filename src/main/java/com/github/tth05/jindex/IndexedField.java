package com.github.tth05.jindex;

public class IndexedField extends ClassChildObject {

    private IndexedField(long classIndexPointer, long classPointer, long pointer) {
        super(classIndexPointer, classPointer, pointer);
    }

    /**
     * @return The name of this field
     */
    public String getName() {
        return executeWhileOwnerOpen(this::getNameNative);
    }

    /**
     * @return The modifiers of this field
     */
    public int getAccessFlags() {
        return executeWhileOwnerOpen(this::getAccessFlagsNative);
    }

    /**
     * @return The descriptor of this field's type
     */
    public String getDescriptorString() {
        return executeWhileOwnerOpen(this::getDescriptorStringNative);
    }

    /**
     * @return The generic signature of this field, or {@code null} if this field's type is not generic
     */
    public String getGenericSignatureString() {
        return executeWhileOwnerOpen(this::getGenericSignatureStringNative);
    }

    private native String getNameNative();

    private native int getAccessFlagsNative();

    private native String getDescriptorStringNative();

    private native String getGenericSignatureStringNative();

    @Override
    public String toString() {
        return getName();
    }
}
