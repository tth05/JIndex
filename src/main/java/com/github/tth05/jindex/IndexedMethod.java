package com.github.tth05.jindex;

public class IndexedMethod extends ClassChildObject {

    public IndexedMethod(long classIndexPointer, long classPointer, long pointer) {
        super(classIndexPointer, classPointer, pointer);
    }

    /**
     * @return The class of which this method is a member of
     */
    public IndexedClass getDeclaringClass() {
        return executeWhileOwnerOpen(this::getDeclaringClassNative);
    }

    /**
     * @return The name of this method
     */
    public String getName() {
        return executeWhileOwnerOpen(this::getNameNative);
    }

    /**
     * @return The modifiers of this method
     */
    public int getAccessFlags() {
        return executeWhileOwnerOpen(this::getAccessFlagsNative);
    }

    /**
     * @return The descriptor of this method
     */
    public String getDescriptorString() {
        return executeWhileOwnerOpen(this::getDescriptorStringNative);
    }

    /**
     * @return The generic signature of this method, or {@code null} if it has none
     */
    public String getGenericSignatureString() {
        return executeWhileOwnerOpen(this::getGenericSignatureStringNative);
    }

    /**
     * @return The exceptions of this method which are found in the 'Exceptions' attribute of a method in a class file,
     * or an empty array if there are none
     */
    public IndexedClass[] getExceptions() {
        return executeWhileOwnerOpen(this::getExceptionsNative);
    }

    /**
     * Searches all methods of all classes to find the ones which override this method. Use {@link #findBaseMethods()}
     * to search for implementations of this method's base method instead.
     *
     * @return The methods which override this method, or an empty array if there are none
     */
    public IndexedMethod[] findImplementations() {
        return executeWhileOwnerOpen(this::findImplementationsNative);
    }

    /**
     * Searches all methods of all classes to find the ones which this method overrides. If the hierarchy has multiple
     * levels, all methods down to the actual base methods will be returned
     *
     * @return The base methods of this method, or an empty array if there are none
     */
    public IndexedMethod[] findBaseMethods() {
        return executeWhileOwnerOpen(this::findBaseMethodsNative);
    }

    private native IndexedClass getDeclaringClassNative();

    private native String getNameNative();

    private native int getAccessFlagsNative();

    private native String getDescriptorStringNative();

    private native String getGenericSignatureStringNative();

    private native IndexedClass[] getExceptionsNative();

    private native IndexedMethod[] findImplementationsNative();

    private native IndexedMethod[] findBaseMethodsNative();

    @Override
    public String toString() {
        return getName();
    }
}
