package com.github.tth05.jindex;

public class IndexedPackage extends ClassIndexChildObject {

    public IndexedPackage(long classIndexPointer, long pointer) {
        super(classIndexPointer, pointer);
    }

    /**
     * @return The name of this package part
     */
    public String getName() {
        return executeWhileOwnerOpen(this::getNameNative);
    }

    /**
     * @return The name of this package including all parents
     */
    public String getNameWithParents() {
        return executeWhileOwnerOpen(this::getNameWithParentsNative);
    }

    /**
     * @return Same as {@link #getNameWithParents()}, but using '.' as the package separator
     */
    public String getNameWithParentsDot() {
        return executeWhileOwnerOpen(this::getNameWithParentsDotNative);
    }

    /**
     * @return All classes which are members of this package, or an empty array if there are none
     */
    public IndexedClass[] getClasses() {
        return executeWhileOwnerOpen(this::getClassesNative);
    }

    /**
     * @return All packages which are members of this package, or an empty array if there are none
     */
    public IndexedPackage[] getSubPackages() {
        return executeWhileOwnerOpen(this::getSubPackagesNative);
    }

    private native String getNameNative();

    private native String getNameWithParentsNative();

    private native String getNameWithParentsDotNative();

    private native IndexedClass[] getClassesNative();

    private native IndexedPackage[] getSubPackagesNative();
}
