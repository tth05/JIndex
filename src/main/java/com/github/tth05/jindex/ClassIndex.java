package com.github.tth05.jindex;

import java.io.IOException;
import java.io.InputStream;
import java.lang.ref.Cleaner;
import java.lang.ref.WeakReference;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.EnumSet;
import java.util.List;
import java.util.Objects;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ConcurrentMap;
import java.util.concurrent.locks.ReentrantReadWriteLock;
import java.util.function.IntSupplier;
import java.util.function.Supplier;

public class ClassIndex extends ClassIndexChildObject implements AutoCloseable {

    private static final Cleaner CLEANER = Cleaner.create();
    private static final ConcurrentMap<Long, WeakReference<ClassIndex>> OWNERS = new ConcurrentHashMap<>();

    static {
        try (InputStream nativeLibrary = Objects.requireNonNull(
                ClassIndex.class.getResourceAsStream("/jindex_natives/jindex_rs.dll"),
                "Missing bundled jindex native library"
        )) {
            NativeLibraryLoader.load(nativeLibrary, NativeLibraryLoader.cacheDirectory());
        } catch (IOException e) {
            throw new RuntimeException("Unable to load native library", e);
        }
    }

    private volatile boolean destroyed;
    private BuildTimeInfo buildTimeInfo;
    private Cleaner.Cleanable cleanable;
    private final ReentrantReadWriteLock lifecycleLock = new ReentrantReadWriteLock(true);

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
        Objects.requireNonNull(packageName, "packageName");
        Objects.requireNonNull(className, "className");
        return executeWhileOpen(() -> findClassNative(packageName, className));
    }

    /**
     * Returns the class with the exact binary name. Both {@code '.'} and {@code '/'} may be used as package
     * separators; nested classes retain their {@code '$'} binary-name separator.
     *
     * @param binaryName the exact binary name, for example {@code java.lang.String} or
     *                   {@code java.util.Map$Entry}
     * @return the class, or {@code null} when the binary name is not indexed
     */
    public IndexedClass findClass(String binaryName) {
        Objects.requireNonNull(binaryName, "binaryName");
        int separator = Math.max(binaryName.lastIndexOf('.'), binaryName.lastIndexOf('/'));
        String packageName = separator < 0 ? "" : binaryName.substring(0, separator);
        String className = binaryName.substring(separator + 1);
        return findClass(packageName, className);
    }

    /**
     * Returns a bounded page of classes whose simple name matches the query. A package-qualified query restricts the
     * search to that exact package. Both {@code '.'} and {@code '/'} may be used as package separators.
     *
     * @param query   The query to search for
     * @param options The search options
     * @return matching classes and whether the configured limit omitted additional matches
     */
    public ClassSearchPage findClasses(String query, SearchOptions options) {
        Objects.requireNonNull(query, "query");
        Objects.requireNonNull(options, "options");
        return executeWhileOpen(() -> findClasses0(query, options, null));
    }

    /**
     * Returns matching classes declared by the selected sources. Passing no source IDs selects no sources; use
     * {@link #findClasses(String, SearchOptions)} for all sources.
     *
     * @param query the class-name query
     * @param options matching and limit options
     * @param sourceIds opaque source IDs to include
     * @return matching classes and whether the configured limit omitted additional matches
     */
    public ClassSearchPage findClasses(String query, SearchOptions options, int... sourceIds) {
        Objects.requireNonNull(query, "query");
        Objects.requireNonNull(options, "options");
        int[] normalizedSourceIds = normalizeSourceIds(sourceIds);
        return executeWhileOpen(() -> findClasses0(query, options, normalizedSourceIds));
    }

    /**
     * Searches complete binary names instead of only simple class names. Both {@code '.'} and {@code '/'} may be
     * used as package separators; nested classes retain their {@code '$'} separator.
     *
     * @param query the complete-binary-name query
     * @param options matching and limit options
     * @return matching classes and whether the configured limit omitted additional matches
     */
    public ClassSearchPage findClassesByBinaryName(String query, SearchOptions options) {
        Objects.requireNonNull(query, "query");
        Objects.requireNonNull(options, "options");
        return executeWhileOpen(() -> findClassesByBinaryName0(query, options, null));
    }

    /**
     * Searches complete binary names from the selected sources. Passing no source IDs selects no sources; use
     * {@link #findClassesByBinaryName(String, SearchOptions)} for all sources.
     *
     * @param query the complete-binary-name query
     * @param options matching and limit options
     * @param sourceIds opaque source IDs to include
     * @return matching classes and whether the configured limit omitted additional matches
     */
    public ClassSearchPage findClassesByBinaryName(
            String query,
            SearchOptions options,
            int... sourceIds
    ) {
        Objects.requireNonNull(query, "query");
        Objects.requireNonNull(options, "options");
        int[] normalizedSourceIds = normalizeSourceIds(sourceIds);
        return executeWhileOpen(() -> findClassesByBinaryName0(query, options, normalizedSourceIds));
    }

    private ClassSearchPage findClassesByBinaryName0(
            String query,
            SearchOptions options,
            int[] sourceIds
    ) {
        if (query.isEmpty()) {
            return new ClassSearchPage(new IndexedClass[0], false);
        }
        return findClassesByBinaryNameNative(query.replace('.', '/'), options, sourceIds);
    }

    private ClassSearchPage findClasses0(String query, SearchOptions options, int[] sourceIds) {
        if (query.isEmpty()) {
            return new ClassSearchPage(new IndexedClass[0], false);
        }
        return findClassesNative(query.replace('.', '/'), options, sourceIds);
    }

    /**
     * <p>Searches for a package which exactly matches the given name. Both '/' and '.' may be used as package
     * separators.</p>
     *
     * @param packageName The package name to search for
     * @return The package, or {@code null} if no package with the given name was found
     */
    public IndexedPackage findPackage(String packageName) {
        return executeWhileOpen(() -> findPackageNative(packageName));
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
        return executeWhileOpen(() -> findPackagesNative(query));
    }

    /**
     * Searches field and method declarations in one native call. Results retain the exact JVM descriptor and the
     * opaque source ID selected for the declaring class.
     *
     * @param query the member-name query
     * @param options matching and limit options
     * @param kinds field and method kinds to include
     * @return deterministically ordered matching declarations and truncation state
     */
    public SymbolSearchPage findSymbols(
            String query,
            SearchOptions options,
            EnumSet<SymbolKind> kinds
    ) {
        Objects.requireNonNull(query, "query");
        Objects.requireNonNull(options, "options");
        int kindMask = symbolKindMask(kinds);
        return executeWhileOpen(() -> findSymbolsNative(query, options, kindMask, null));
    }

    /**
     * Searches field and method declarations from the selected sources. Passing no source IDs selects no sources;
     * use {@link #findSymbols(String, SearchOptions, EnumSet)} for all sources.
     *
     * @param query the member-name query
     * @param options matching and limit options
     * @param kinds field and method kinds to include
     * @param sourceIds opaque source IDs to include
     * @return deterministically ordered matching declarations and truncation state for the selected sources
     */
    public SymbolSearchPage findSymbols(
            String query,
            SearchOptions options,
            EnumSet<SymbolKind> kinds,
            int... sourceIds
    ) {
        Objects.requireNonNull(query, "query");
        Objects.requireNonNull(options, "options");
        int kindMask = symbolKindMask(kinds);
        int[] normalizedSourceIds = normalizeSourceIds(sourceIds);
        return executeWhileOpen(() -> findSymbolsNative(query, options, kindMask, normalizedSourceIds));
    }

    private static int symbolKindMask(EnumSet<SymbolKind> kinds) {
        Objects.requireNonNull(kinds, "kinds");
        int kindMask = 0;
        if (kinds.contains(SymbolKind.FIELD)) {
            kindMask |= 1;
        }
        if (kinds.contains(SymbolKind.METHOD)) {
            kindMask |= 2;
        }
        return kindMask;
    }

    public IndexStatistics getStatistics() {
        return executeWhileOpen(this::getStatisticsNative);
    }

    ReferenceStorageStatistics getReferenceStorageStatistics() {
        return executeWhileOpen(this::getReferenceStorageStatisticsNative);
    }

    /**
     * Returns a bounded page of indexed declaration sites that reference the target.
     *
     * @param target exact declaration whose incoming references should be returned
     * @param limit maximum number of declaration sites to return
     * @return the matching declaration sites and truncation state
     */
    public ReferenceSearchPage findReferences(ReferenceTarget target, int limit) {
        Objects.requireNonNull(target, "target");
        requirePositiveLimit(limit);
        return executeWhileOpen(() -> findReferencesNative(
                target.kind().ordinal(),
                target.ownerInternalName(),
                target.name(),
                target.descriptor(),
                null,
                limit
        ));
    }

    /**
     * Returns aggregate incoming-reference counts without allocating one result object per declaration site.
     *
     * @param target exact declaration whose incoming references should be counted
     * @return distinct declaration-site and total occurrence counts
     */
    public ReferenceSummary summarizeReferences(ReferenceTarget target) {
        Objects.requireNonNull(target, "target");
        return executeWhileOpen(() -> summarizeReferencesNative(
                target.kind().ordinal(),
                target.ownerInternalName(),
                target.name(),
                target.descriptor()
        ));
    }

    /**
     * Returns a bounded page of indexed declaration sites from the selected sources that reference the target.
     * Passing no source IDs selects no sources; use {@link #findReferences(ReferenceTarget, int)} for all sources.
     *
     * @param target exact declaration whose incoming references should be returned
     * @param limit maximum number of declaration sites to return
     * @param sourceIds opaque source IDs to include
     * @return the matching declaration sites and truncation state
     */
    public ReferenceSearchPage findReferences(ReferenceTarget target, int limit, int... sourceIds) {
        Objects.requireNonNull(target, "target");
        requirePositiveLimit(limit);
        int[] normalizedSourceIds = normalizeSourceIds(sourceIds);
        return executeWhileOpen(() -> findReferencesNative(
                target.kind().ordinal(),
                target.ownerInternalName(),
                target.name(),
                target.descriptor(),
                normalizedSourceIds,
                limit
        ));
    }

    /**
     * Returns a bounded page of indexed declaration sites containing the exact Java string literal.
     *
     * @param literal exact Java string value
     * @param limit maximum number of declaration sites to return
     * @return the matching declaration sites and truncation state
     */
    public ReferenceSearchPage findLiteralReferences(String literal, int limit) {
        Objects.requireNonNull(literal, "literal");
        requirePositiveLimit(limit);
        return executeWhileOpen(() -> findLiteralReferencesNative(literal, null, limit));
    }

    /**
     * Returns a bounded page of indexed declaration sites from the selected sources containing the exact Java
     * string literal. Passing no source IDs selects no sources; use {@link #findLiteralReferences(String, int)} for
     * all sources.
     *
     * @param literal exact Java string value
     * @param limit maximum number of declaration sites to return
     * @param sourceIds opaque source IDs to include
     * @return the matching declaration sites and truncation state
     */
    public ReferenceSearchPage findLiteralReferences(String literal, int limit, int... sourceIds) {
        Objects.requireNonNull(literal, "literal");
        requirePositiveLimit(limit);
        int[] normalizedSourceIds = normalizeSourceIds(sourceIds);
        return executeWhileOpen(() -> findLiteralReferencesNative(literal, normalizedSourceIds, limit));
    }

    /**
     * Finds distinct Java string values containing the exact UTF-16 query.
     *
     * @param query exact UTF-16 substring to match
     * @param limit maximum number of values to return
     * @return the matching values and truncation state
     */
    public LiteralSearchPage findLiteralsContaining(String query, int limit) {
        Objects.requireNonNull(query, "query");
        requirePositiveLimit(limit);
        return executeWhileOpen(() -> findLiteralsContainingNative(query, null, limit));
    }

    /**
     * Finds distinct Java string values containing the exact UTF-16 query in the selected sources. Passing no source
     * IDs selects no sources; use {@link #findLiteralsContaining(String, int)} for all sources.
     *
     * @param query exact UTF-16 substring to match
     * @param limit maximum number of values to return
     * @param sourceIds opaque source IDs to include
     * @return matching values, their selected source IDs, and truncation state
     */
    public LiteralSearchPage findLiteralsContaining(String query, int limit, int... sourceIds) {
        Objects.requireNonNull(query, "query");
        requirePositiveLimit(limit);
        int[] normalizedSourceIds = normalizeSourceIds(sourceIds);
        return executeWhileOpen(() -> findLiteralsContainingNative(query, normalizedSourceIds, limit));
    }

    private static void requirePositiveLimit(int limit) {
        if (limit < 1) {
            throw new IllegalArgumentException("limit must be positive");
        }
    }

    private static int[] normalizeSourceIds(int[] sourceIds) {
        Objects.requireNonNull(sourceIds, "sourceIds");
        int[] normalized = sourceIds.clone();
        for (int sourceId : normalized) {
            if (sourceId < 0) {
                throw new IllegalArgumentException("sourceIds must not contain negative values");
            }
        }
        Arrays.sort(normalized);
        if (normalized.length < 2) {
            return normalized;
        }
        int uniqueCount = 1;
        for (int index = 1; index < normalized.length; index++) {
            if (normalized[index] != normalized[uniqueCount - 1]) {
                normalized[uniqueCount++] = normalized[index];
            }
        }
        return Arrays.copyOf(normalized, uniqueCount);
    }

    public void saveToFile(String filePath) {
        executeWhileOpen(() -> saveToFileNative(filePath));
    }

    /**
     * Drops all natively managed memory used by this class index. This method is idempotent. Objects obtained from
     * this index must not be used after it is destroyed.
     */
    public void destroy() {
        close();
    }

    @Override
    public void close() {
        var writeLock = this.lifecycleLock.writeLock();
        writeLock.lock();
        try {
            if (this.destroyed) {
                return;
            }

            this.destroyed = true;
            this.cleanable.clean();
            clearClassIndexPointer();
        } finally {
            writeLock.unlock();
        }
    }

    /**
     * @return {@code true} if this class index has been destroyed and is deemed unusable, {@code false} otherwise
     */
    public boolean isDestroyed() {
        return destroyed;
    }

    private native BuildTimeInfo createClassIndexFromBytes(List<byte[]> classes);

    private native BuildTimeInfo createClassIndexFromJars(List<String> classes, int targetJavaRelease);

    private native BuildTimeInfo createClassIndexFromExplicitSources(
            List<String> jarFilePaths,
            int[] jarSourceIds,
            int[] jarInputOrders,
            List<byte[]> classes,
            int[] classSourceIds,
            int[] classInputOrders,
            int targetJavaRelease
    );

    private native BuildTimeInfo loadClassIndexFromFile(String filePath);

    private native IndexedClass findClassNative(String packageName, String className);

    private native ClassSearchPage findClassesNative(String query, SearchOptions options, int[] sourceIds);

    private native ClassSearchPage findClassesByBinaryNameNative(
            String query,
            SearchOptions options,
            int[] sourceIds
    );

    private native SymbolSearchPage findSymbolsNative(
            String query,
            SearchOptions options,
            int kindMask,
            int[] sourceIds
    );

    private native IndexStatistics getStatisticsNative();

    private native ReferenceStorageStatistics getReferenceStorageStatisticsNative();

    private native ReferenceSearchPage findReferencesNative(
            int targetKind,
            String ownerInternalName,
            String name,
            String descriptor,
            int[] sourceIds,
            int limit
    );

    private native ReferenceSummary summarizeReferencesNative(
            int targetKind,
            String ownerInternalName,
            String name,
            String descriptor
    );

    private native ReferenceSearchPage findLiteralReferencesNative(String literal, int[] sourceIds, int limit);

    private native LiteralSearchPage findLiteralsContainingNative(String query, int[] sourceIds, int limit);

    private native IndexedPackage findPackageNative(String packageName);

    private native IndexedPackage[] findPackagesNative(String query);

    private native void saveToFileNative(String filePath);

    private static native void destroyPointer(long pointer);

    private void ensureOpen() {
        if (this.destroyed) {
            throw new IllegalStateException("Class index is closed");
        }
    }

    final <T> T executeWhileOpen(Supplier<T> operation) {
        Objects.requireNonNull(operation, "operation");
        var readLock = this.lifecycleLock.readLock();
        readLock.lock();
        try {
            ensureOpen();
            return operation.get();
        } finally {
            readLock.unlock();
        }
    }

    final int executeWhileOpen(IntSupplier operation) {
        Objects.requireNonNull(operation, "operation");
        var readLock = this.lifecycleLock.readLock();
        readLock.lock();
        try {
            ensureOpen();
            return operation.getAsInt();
        } finally {
            readLock.unlock();
        }
    }

    final void executeWhileOpen(Runnable operation) {
        Objects.requireNonNull(operation, "operation");
        var readLock = this.lifecycleLock.readLock();
        readLock.lock();
        try {
            ensureOpen();
            operation.run();
        } finally {
            readLock.unlock();
        }
    }

    private void registerCleanup() {
        long pointer = classIndexPointer();
        if (pointer == 0) {
            throw new IllegalStateException("The native class index was not initialized");
        }

        WeakReference<ClassIndex> ownerReference = new WeakReference<>(this);
        OWNERS.put(pointer, ownerReference);
        NativeCleanup cleanup = new NativeCleanup(pointer, ownerReference);
        try {
            this.cleanable = CLEANER.register(this, cleanup);
            this.destroyed = false;
        } catch (RuntimeException | Error e) {
            cleanup.run();
            clearClassIndexPointer();
            throw e;
        }
    }

    static ClassIndex ownerFor(long pointer) {
        WeakReference<ClassIndex> reference = OWNERS.get(pointer);
        ClassIndex owner = reference == null ? null : reference.get();
        if (owner == null) {
            throw new IllegalStateException("The native class index owner is unavailable");
        }
        return owner;
    }

    private record NativeCleanup(long pointer, WeakReference<ClassIndex> ownerReference) implements Runnable {
        @Override
        public void run() {
            if (OWNERS.remove(this.pointer, this.ownerReference)) {
                destroyPointer(this.pointer);
            }
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
        c.buildTimeInfo = c.createClassIndexFromJars(
                jarFilePaths,
                IndexBuildOptions.currentRuntime().targetJavaRelease()
        );
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
     * Creates a class index from archive paths and direct class-file bytes. Direct class files take precedence when
     * an archive contains a class with the same internal name.
     *
     * @param jarFilePaths The JAR or ZIP file paths to index
     * @param classes      The direct class-file bytes to index
     * @return The class index
     */
    public static ClassIndex fromSources(List<String> jarFilePaths, List<byte[]> classes) {
        Objects.requireNonNull(jarFilePaths, "jarFilePaths");
        Objects.requireNonNull(classes, "classes");
        List<IndexSource> sources = new ArrayList<>(Math.addExact(jarFilePaths.size(), classes.size()));
        for (int index = 0; index < classes.size(); index++) {
            sources.add(IndexSource.classFile(Math.addExact(jarFilePaths.size(), index), classes.get(index)));
        }
        for (int index = 0; index < jarFilePaths.size(); index++) {
            sources.add(IndexSource.archive(index, jarFilePaths.get(index)));
        }
        return fromSources(sources);
    }

    /**
     * Creates a class index from ordered inputs with caller-owned source IDs. Earlier inputs win
     * when multiple sources define the same class. A source ID may be shared by any number of
     * archive and class-file inputs.
     *
     * @param sources inputs in descending precedence order
     * @return the class index
     */
    public static ClassIndex fromSources(List<? extends IndexSource> sources) {
        return fromSources(sources, IndexBuildOptions.currentRuntime());
    }

    /**
     * Creates a class index from ordered inputs using the requested Java runtime view. Multi-release archives select
     * the highest versioned class no newer than {@link IndexBuildOptions#targetJavaRelease()}.
     *
     * @param sources inputs in descending precedence order
     * @param options runtime-view options
     * @return the class index
     */
    public static ClassIndex fromSources(
            List<? extends IndexSource> sources,
            IndexBuildOptions options
    ) {
        Objects.requireNonNull(sources, "sources");
        Objects.requireNonNull(options, "options");
        List<? extends IndexSource> orderedSources = List.copyOf(sources);
        int archiveCount = 0;
        int classCount = 0;
        for (IndexSource source : orderedSources) {
            switch (source) {
                case IndexSource.Archive ignored -> archiveCount++;
                case IndexSource.ClassFile ignored -> classCount++;
            }
        }

        List<String> jarFilePaths = new ArrayList<>(archiveCount);
        int[] jarSourceIds = new int[archiveCount];
        int[] jarInputOrders = new int[archiveCount];
        List<byte[]> classes = new ArrayList<>(classCount);
        int[] classSourceIds = new int[classCount];
        int[] classInputOrders = new int[classCount];
        int archiveIndex = 0;
        int classIndex = 0;
        for (int inputOrder = 0; inputOrder < orderedSources.size(); inputOrder++) {
            switch (orderedSources.get(inputOrder)) {
                case IndexSource.Archive archive -> {
                    jarFilePaths.add(archive.path());
                    jarSourceIds[archiveIndex] = archive.sourceId();
                    jarInputOrders[archiveIndex] = inputOrder;
                    archiveIndex++;
                }
                case IndexSource.ClassFile classFile -> {
                    classes.add(classFile.bytes());
                    classSourceIds[classIndex] = classFile.sourceId();
                    classInputOrders[classIndex] = inputOrder;
                    classIndex++;
                }
            }
        }

        ClassIndex c = new ClassIndex();
        c.buildTimeInfo = c.createClassIndexFromExplicitSources(
                jarFilePaths,
                jarSourceIds,
                jarInputOrders,
                classes,
                classSourceIds,
                classInputOrders,
                options.targetJavaRelease()
        );
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
