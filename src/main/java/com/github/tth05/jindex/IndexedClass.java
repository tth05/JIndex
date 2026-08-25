package com.github.tth05.jindex;

import java.util.Arrays;

public class IndexedClass extends ClassIndexChildObject {

    private IndexedClass(long classIndexPointer, long pointer) {
        super(classIndexPointer, pointer);
    }

    /**
     * @return The full name of the class, e.g. "String", "String$1LocalClass", "String$1$2$Class"
     */
    public String getName() {
        return executeWhileOwnerOpen(this::getNameNative);
    }

    /**
     * Returns the name of the class as it appears in the source code. The name is extracted from the inner class
     * attributes of this class. {@link #getInnerClassType()} can be used to check for anonymous classes if they are
     * expected to not have a source name.
     * <br>
     * For example, all of these could be valid names that cannot always be determined using a split on the last '$'
     * <br>
     * <ul>
     *      <li>"String" : "String"</li>
     *      <li>"String$1LocalClass" : "LocalClass"</li>
     *      <li>"String$1$2$Class" : "2$Class"</li>
     * </ul>
     *
     * @return The name of the class as it appears in the source code
     */
    public String getSourceName() {
        return executeWhileOwnerOpen(this::getSourceNameNative);
    }

    /**
     * <p>The package of this class. If this class is not in a package, this will be the empty package which has an empty
     * name.</p>
     *
     * @return The package of this class
     */
    public IndexedPackage getPackage() {
        return executeWhileOwnerOpen(this::getPackageNative);
    }

    /**
     * @return The name of this class including the package, e.g. "java/lang/String"
     */
    public String getNameWithPackage() {
        return executeWhileOwnerOpen(this::getNameWithPackageNative);
    }

    /**
     * @return The same as {@link #getNameWithPackage()}, but using '.' as the package separator
     */
    public String getNameWithPackageDot() {
        return executeWhileOwnerOpen(this::getNameWithPackageDotNative);
    }

    /**
     * @return The generic signature of this class as it may be found in the 'Signature' attribute of a class file, or
     * {@code null} if this class does not have a generic signature
     */
    public String getGenericSignatureString() {
        return executeWhileOwnerOpen(this::getGenericSignatureStringNative);
    }

    /**
     * @return The enclosing class of this class, or {@code null} if this class is not an inner class
     */
    public IndexedClass getEnclosingClass() {
        return executeWhileOwnerOpen(this::getEnclosingClassNative);
    }

    /**
     * @return The enclosing method name and descriptor of this class, or {@code null} if this class is not enclosed by
     * a method. The returned value might look like this: <code>foo(Ljava/lang/String;)V</code>
     */
    public String getEnclosingMethodNameAndDesc() {
        return executeWhileOwnerOpen(this::getEnclosingMethodNameAndDescNative);
    }

    /**
     * @return The inner class type of this class, or {@code null} if this class is not an inner class
     */
    public InnerClassType getInnerClassType() {
        int type = executeWhileOwnerOpen(this::getInnerClassType0);
        if (type < 0)
            return null;

        return InnerClassType.values()[type];
    }

    private native int getInnerClassType0();

    /**
     * @return All inner classes of this class with the {@link InnerClassType#MEMBER} type
     */
    public IndexedClass[] getMemberClasses() {
        return executeWhileOwnerOpen(this::getMemberClassesNative);
    }

    /**
     * Returns all class which implemented this class.
     *
     * @param directSubTypesOnly Whether to only return direct subtypes or not
     * @return All classes which implemented this class, or an empty array if none were found
     */
    public IndexedClass[] findImplementations(boolean directSubTypesOnly) {
        return executeWhileOwnerOpen(() -> findImplementationsNative(directSubTypesOnly));
    }

    /**
     * Counts implementations of this class and of each method declared by it in one native query.
     *
     * @return hierarchy counts aligned with {@link #getMethods()}
     */
    public HierarchySummary summarizeHierarchy() {
        return executeWhileOwnerOpen(this::summarizeHierarchyNative);
    }

    /**
     * @return The super class of this class, or {@code null} if this class is {@code java/lang/Object} or if the super class is unresolved
     */
    public IndexedClass getSuperClass() {
        return executeWhileOwnerOpen(this::getSuperClassNative);
    }

    /**
     * @return The interfaces implemented by this class, or an empty array if this class does not implement any
     * interfaces
     */
    public IndexedClass[] getInterfaces() {
        return executeWhileOwnerOpen(this::getInterfacesNative);
    }

    /**
     * @return The fields of this class
     */
    public IndexedField[] getFields() {
        return executeWhileOwnerOpen(this::getFieldsNative);
    }

    /**
     * @return The methods of this class
     */
    public IndexedMethod[] getMethods() {
        return executeWhileOwnerOpen(this::getMethodsNative);
    }

    /**
     * @return The modifiers of this class
     */
    public int getAccessFlags() {
        return executeWhileOwnerOpen(this::getAccessFlagsNative);
    }

    /**
     * @return the opaque ID of the input source that supplied this class
     */
    public int getSourceId() {
        return executeWhileOwnerOpen(this::getSourceIdNative);
    }

    private native String getNameNative();

    private native String getSourceNameNative();

    private native IndexedPackage getPackageNative();

    private native String getNameWithPackageNative();

    private native String getNameWithPackageDotNative();

    private native String getGenericSignatureStringNative();

    private native IndexedClass getEnclosingClassNative();

    private native String getEnclosingMethodNameAndDescNative();

    private native IndexedClass[] getMemberClassesNative();

    private native IndexedClass[] findImplementationsNative(boolean directSubTypesOnly);

    private native HierarchySummary summarizeHierarchyNative();

    private native IndexedClass getSuperClassNative();

    private native IndexedClass[] getInterfacesNative();

    private native IndexedField[] getFieldsNative();

    private native IndexedMethod[] getMethodsNative();

    private native int getAccessFlagsNative();

    private native int getSourceIdNative();

    @Override
    public String toString() {
        return "FindClassesResult{" +
               "className='" + getNameWithPackage() + '\'' +
               ", methodNames=" + Arrays.toString(getMethods()) +
               '}';
    }
}
