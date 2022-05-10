package com.github.tth05.jindex;

public class IndexedField extends ClassChildObject {

    private IndexedField(long classIndexPointer, long classPointer, long pointer) {
        super(classIndexPointer, classPointer, pointer);
    }

    /**
     * @return The name of this field
     */
    public native String getName();

    /**
     * @return The modifiers of this field
     */
    public native int getAccessFlags();

    /**
     * @return The descriptor of this field's type
     */
    public native String getDescriptorString();

    /**
     * @return The generic signature of this field, or {@code null} if this field's type is not generic
     */
    public native String getGenericSignatureString();

    @Override
    public String toString() {
        return getName();
    }
}
