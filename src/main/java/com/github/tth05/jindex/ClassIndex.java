package com.github.tth05.jindex;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.List;
import java.util.Objects;

public class ClassIndex extends ClassIndexChildObject {

    static {
        try (InputStream nativeLibrary = Objects.requireNonNull(
                ClassIndex.class.getResourceAsStream("/jindex_natives/jindex_rs.dll"),
                "Missing bundled jindex native library"
        )) {
            Path extractedLibrary = Files.createTempFile("jindex_rs-", ".dll");
            Files.copy(nativeLibrary, extractedLibrary, StandardCopyOption.REPLACE_EXISTING);
            extractedLibrary.toFile().deleteOnExit();
            System.load(extractedLibrary.toAbsolutePath().toString());
        } catch (IOException e) {
            throw new RuntimeException("Unable to load native library", e);
        }
    }

    private boolean destroyed;
    private BuildTimeInfo buildTimeInfo;

    private ClassIndex() {
        super(0, 0);
        this.destroyed = true;
    }

    /**
     * <p>Returns the class in the given package with the given name.</p>
     *
     * @param packageName The package name
     * @param className   The class name
     * @return The class, or {@code null} if no class matching the input was found
     */
    public native IndexedClass findClass(String packageName, String className);

    /**
     * <p>Returns an array of classes which match the given query and the given search options.</p>
     *
     * @param query   The query to search for
     * @param options The search options
     * @return The classes which match the query and options, or an empty array if no classes were found
     */
    public native IndexedClass[] findClasses(String query, SearchOptions options);

    /**
     * <p>Searches for a package which exactly matches the given name. Both '/' and '.' may be used as package
     * separators.</p>
     *
     * @param packageName The package name to search for
     * @return The package, or {@code null} if no package with the given name was found
     */
    public native IndexedPackage findPackage(String packageName);

    /**
     * <p>Returns an array of packages which start with the given query. The query is case sensitive. Both '/' and '.'
     * may be used as package separators.</p>
     * Examples:
     * <blockquote><pre>
     *     findPackages("java") - ["java"]
     *     findPackages("java.a") - ["java.awt", "java.applet"]
     *     findPackages("java.") - ["java.awt", "java.applet", "java.beans", ..., "java.util"]
     * </pre></blockquote>
     *
     * @param query The query to search for
     * @return An array of packages, or an empty array if no packages were found
     */
    public native IndexedPackage[] findPackages(String query);

    public List<String> findMethods(String query, int limit) {
        throw new UnsupportedOperationException();
    }

    public native void saveToFile(String filePath);

    /**
     * Drops all natively managed memory used by this class index. Any further attempt to use this class index will
     * result in a JVM crash.
     */
    public native void destroy();

    /**
     * @return {@code true} if this class index has been destroyed and is deemed unusable, {@code false} otherwise
     */
    public boolean isDestroyed() {
        return destroyed;
    }

    private native BuildTimeInfo createClassIndexFromBytes(List<byte[]> classes);

    private native BuildTimeInfo createClassIndexFromJars(List<String> classes);

    private native BuildTimeInfo loadClassIndexFromFile(String filePath);

    @Override
    protected void finalize() {
        if (this.destroyed)
            return;

        destroy();
    }

    /**
     * @return The build time information for this class index
     */
    public BuildTimeInfo getBuildTimeInfo() {
        return this.buildTimeInfo;
    }

    /**
     * Creates a new ClassIndex from the given jar file path.
     *
     * @param jarFilePaths The jar file paths to index
     * @return The class index
     */
    public static ClassIndex fromJars(List<String> jarFilePaths) {
        ClassIndex c = new ClassIndex();
        c.buildTimeInfo = c.createClassIndexFromJars(jarFilePaths);
        c.destroyed = false;
        return c;
    }

    /**
     * Creates a class index from a list of class files.
     *
     * @param classes The list of class files
     * @return The class index
     */
    public static ClassIndex fromBytes(List<byte[]> classes) {
        ClassIndex c = new ClassIndex();
        c.buildTimeInfo = c.createClassIndexFromBytes(classes);
        c.destroyed = false;
        return c;
    }

    /**
     * Loads a class index from a save file. This save file should have been created using {@link #saveToFile(String)}.
     *
     * @param path The path to the save file
     * @return The deserialized class index
     */
    public static ClassIndex fromFile(String path) {
        ClassIndex c = new ClassIndex();
        c.buildTimeInfo = c.loadClassIndexFromFile(path);
        c.destroyed = false;
        return c;
    }
}
