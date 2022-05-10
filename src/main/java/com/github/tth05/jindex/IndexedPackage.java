package com.github.tth05.jindex;

public class IndexedPackage extends ClassIndexChildObject{

    public IndexedPackage(long classIndexPointer, long pointer) {
        super(classIndexPointer, pointer);
    }

    /**
     * @return The name of this package part
     */
    public native String getName();

    /**
     * @return The name of this package including all parents
     */
    public native String getNameWithParents();

    /**
     * @return Same as {@link #getNameWithParents()}, but using '.' as the package separator
     */
    public native String getNameWithParentsDot();

    /**
     * @return All classes which are members of this package, or an empty array if there are none
     */
    public native IndexedClass[] getClasses();

    /**
     * @return All packages which are members of this package, or an empty array if there are none
     */
    public native IndexedPackage[] getSubPackages();
}
