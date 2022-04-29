package com.github.tth05.jindex;

import java.util.Arrays;

public class IndexedClass extends ClassIndexChildObject {

    public IndexedClass(long classIndexPointer, long pointer) {
        super(classIndexPointer, pointer);
    }

    /**
     * @return the full name of the class, e.g. "String", "String$1LocalClass", "String$1$2$Class"
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
     * @return the name of the class as it appears in the source code
     */
    public native String getSourceName();

    public native IndexedPackage getPackage();

    public native String getNameWithPackage();

    public native String getNameWithPackageDot();

    public native String getGenericSignatureString();

    public native IndexedClass getEnclosingClass();

    /**
     * @return The inner class type of this class, or {@code null} if this class is not an inner class.
     */
    public InnerClassType getInnerClassType() {
        int type = getInnerClassType0();
        if (type < 0)
            return null;

        return InnerClassType.values()[type];
    }

    private native int getInnerClassType0();

    public native String getEnclosingMethodNameAndDesc();

    /**
     * @return All inner classes of this class with the {@link InnerClassType#MEMBER}.
     */
    public native IndexedClass[] getMemberClasses();

    public native IndexedClass[] findImplementations(boolean directSubTypesOnly);

    public native IndexedClass getSuperClass();

    public native IndexedClass[] getInterfaces();

    public native IndexedField[] getFields();

    public native IndexedMethod[] getMethods();

    public native int getAccessFlags();

    @Override
    public String toString() {
        return "FindClassesResult{" +
               "className='" + getNameWithPackage() + '\'' +
               ", methodNames=" + Arrays.toString(getMethods()) +
               '}';
    }
}
