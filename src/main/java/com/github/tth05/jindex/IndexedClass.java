package com.github.tth05.jindex;

import java.util.Arrays;

public class IndexedClass extends ClassIndexChildObject {

    private IndexedClass(long classIndexPointer, long pointer) {
        super(classIndexPointer, pointer);
    }

    /**
     * @return The full name of the class, e.g. "String", "String$1LocalClass", "String$1$2$Class"
     */
    public native String getName();

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
    public native String getSourceName();

    /**
     * <p>The package of this class. If this class is not in a package, this will be the empty package which has an empty
     * name.</p>
     *
     * @return The package of this class
     */
    public native IndexedPackage getPackage();

    /**
     * @return The name of this class including the package, e.g. "java/lang/String"
     */
    public native String getNameWithPackage();

    /**
     * @return The same as {@link #getNameWithPackage()}, but using '.' as the package separator
     */
    public native String getNameWithPackageDot();

    /**
     * @return The generic signature of this class as it may be found in the 'Signature' attribute of a class file, or
     * {@code null} if this class does not have a generic signature
     */
    public native String getGenericSignatureString();

    /**
     * @return The enclosing class of this class, or {@code null} if this class is not an inner class
     */
    public native IndexedClass getEnclosingClass();

    /**
     * @return The enclosing method name and descriptor of this class, or {@code null} if this class is not enclosed by
     * a method. The returned value might look like this: <code>foo(Ljava/lang/String;)V</code>
     */
    public native String getEnclosingMethodNameAndDesc();

    /**
     * @return The inner class type of this class, or {@code null} if this class is not an inner class
     */
    public InnerClassType getInnerClassType() {
        int type = getInnerClassType0();
        if (type < 0)
            return null;

        return InnerClassType.values()[type];
    }

    private native int getInnerClassType0();

    /**
     * @return All inner classes of this class with the {@link InnerClassType#MEMBER} type
     */
    public native IndexedClass[] getMemberClasses();

    /**
     * Returns all class which implemented this class.
     *
     * @param directSubTypesOnly Whether to only return direct subtypes or not
     * @return All classes which implemented this class, or an empty array if none were found
     */
    public native IndexedClass[] findImplementations(boolean directSubTypesOnly);

    /**
     * @return The super class of this class, or {@code null} if this class is {@code java/lang/Object} or if the super class is unresolved
     */
    public native IndexedClass getSuperClass();

    /**
     * @return The interfaces implemented by this class, or an empty array if this class does not implement any
     * interfaces
     */
    public native IndexedClass[] getInterfaces();

    /**
     * @return The fields of this class
     */
    public native IndexedField[] getFields();

    /**
     * @return The methods of this class
     */
    public native IndexedMethod[] getMethods();

    /**
     * @return The modifiers of this class
     */
    public native int getAccessFlags();

    @Override
    public String toString() {
        return "FindClassesResult{" +
               "className='" + getNameWithPackage() + '\'' +
               ", methodNames=" + Arrays.toString(getMethods()) +
               '}';
    }
}
