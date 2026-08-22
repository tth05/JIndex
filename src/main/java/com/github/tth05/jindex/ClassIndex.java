package com.github.tth05.jindex;

import java.io.IOException;
import java.io.InputStream;
import java.lang.ref.Cleaner;
import java.lang.ref.WeakReference;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.List;
import java.util.Objects;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ConcurrentMap;

public class ClassIndex extends ClassIndexChildObject implements AutoCloseable {

    private static final Cleaner CLEANER = Cleaner.create();
    private static final ConcurrentMap<Long, WeakReference<ClassIndex>> OWNERS = new ConcurrentHashMap<>();

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

    private volatile boolean destroyed;
    private BuildTimeInfo buildTimeInfo;
    private Cleaner.Cleanable cleanable;

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
    public IndexedClass findClass(String packageName, String className) {
        ensureOpen();
        return findClassNative(packageName, className);
    }

    /**
     * <p>Returns an array of classes which match the given query and the given search options.</p>
     *
     * @param query   The query to search for
     * @param options The search options
     * @return The classes which match the query and options, or an empty array if no classes were found
     */
    public IndexedClass[] findClasses(String query, SearchOptions options) {
        ensureOpen();
        return findClassesNative(query, options);
    }

    /**
     * <p>Searches for a package which exactly matches the given name. Both '/' and '.' may be used as package
     * separators.</p>
     *
     * @param packageName The package name to search for
     * @return The package, or {@code null} if no package with the given name was found
     */
    public IndexedPackage findPackage(String packageName) {
        ensureOpen();
        return findPackageNative(packageName);
    }

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
    public IndexedPackage[] findPackages(String query) {
        ensureOpen();
        return findPackagesNative(query);
    }

    public List<String> findMethods(String query, int limit) {
        throw new UnsupportedOperationException();
    }

    public void saveToFile(String filePath) {
        ensureOpen();
        saveToFileNative(filePath);
    }

    /**
     * Drops all natively managed memory used by this class index. This method is idempotent. Objects obtained from
     * this index must not be used after it is destroyed.
     */
    public void destroy() {
        close();
    }

    @Override
    public synchronized void close() {
        if (this.destroyed) {
            return;
        }

        this.destroyed = true;
        long pointer = classIndexPointer();
        OWNERS.remove(pointer);
        this.cleanable.clean();
        clearClassIndexPointer();
    }

    /**
     * @return {@code true} if this class index has been destroyed and is deemed unusable, {@code false} otherwise
     */
    public boolean isDestroyed() {
        return destroyed;
    }

    private native BuildTimeInfo createClassIndexFromBytes(List<byte[]> classes);

    private native BuildTimeInfo createClassIndexFromJars(List<String> classes);

    private native BuildTimeInfo loadClassIndexFromFile(String filePath);

    private native IndexedClass findClassNative(String packageName, String className);

    private native IndexedClass[] findClassesNative(String query, SearchOptions options);

    private native IndexedPackage findPackageNative(String packageName);

    private native IndexedPackage[] findPackagesNative(String query);

    private native void saveToFileNative(String filePath);

    private static native void destroyPointer(long pointer);

    private void ensureOpen() {
        if (this.destroyed) {
            throw new IllegalStateException("This class index has been destroyed");
        }
    }

    private void registerCleanup() {
        long pointer = classIndexPointer();
        if (pointer == 0) {
            throw new IllegalStateException("The native class index was not initialized");
        }

        OWNERS.put(pointer, new WeakReference<>(this));
        this.cleanable = CLEANER.register(this, new NativeCleanup(pointer));
        this.destroyed = false;
    }

    static ClassIndex ownerFor(long pointer) {
        WeakReference<ClassIndex> reference = OWNERS.get(pointer);
        ClassIndex owner = reference == null ? null : reference.get();
        if (owner == null) {
            throw new IllegalStateException("The native class index owner is unavailable");
        }
        return owner;
    }

    private record NativeCleanup(long pointer) implements Runnable {
        @Override
        public void run() {
            destroyPointer(this.pointer);
        }
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
        c.registerCleanup();
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
        c.registerCleanup();
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
        c.registerCleanup();
        return c;
    }
}
