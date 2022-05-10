package com.github.tth05.jindex;

public class IndexedMethod extends ClassChildObject {

    public IndexedMethod(long classIndexPointer, long classPointer, long pointer) {
        super(classIndexPointer, classPointer, pointer);
    }

    /**
     * @return The class of which this method is a member of
     */
    public native IndexedClass getDeclaringClass();

    /**
     * @return The name of this method
     */
    public native String getName();

    /**
     * @return The modifiers of this method
     */
    public native int getAccessFlags();

    /**
     * @return The descriptor of this method
     */
    public native String getDescriptorString();

    /**
     * @return The generic signature of this method, or {@code null} if it has none
     */
    public native String getGenericSignatureString();

    /**
     * @return The exceptions of this method which are found in the 'Exceptions' attribute of a method in a class file,
     * or an empty array if there are none
     */
    public native IndexedClass[] getExceptions();

    /**
     * Searches all methods of all classes to find the ones which override this method. Use {@link #findBaseMethods()}
     * to search for implementations of this method's base method instead.
     *
     * @return The methods which override this method, or an empty array if there are none
     */
    public native IndexedMethod[] findImplementations();

    /**
     * Searches all methods of all classes to find the ones which this method overrides. If the hierarchy has multiple
     * levels, all methods down to the actual base methods will be returned
     *
     * @return The base methods of this method, or an empty array if there are none
     */
    public native IndexedMethod[] findBaseMethods();

    @Override
    public String toString() {
        return getName();
    }
}
