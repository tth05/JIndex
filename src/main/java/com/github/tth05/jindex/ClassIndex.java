package com.github.tth05.jindex;

import java.net.URL;
import java.nio.file.*;
import java.util.List;
import java.util.Optional;
import java.util.stream.Stream;

public class ClassIndex extends ClassIndexChildObject {

    static {
        URL resource = ClassIndex.class.getResource("/lib");
        if (resource == null)
            throw new RuntimeException("Could not find lib directory");

        // Sanitize URL
        boolean isOnDisk = resource.getProtocol().equals("file");
        String actualPath = resource.toString().split("!")[0];
        actualPath = actualPath.substring(actualPath.indexOf("file:") + 6);

        // Either use the default file system (dev environment) or mount the jar file as a file system
        try (FileSystem fileSystem = isOnDisk ? FileSystems.getDefault() : FileSystems.newFileSystem(Paths.get(actualPath), null);
             Stream<Path> fileStream = Files.list(isOnDisk ? fileSystem.getPath(actualPath) : fileSystem.getPath("/"))
        ) {
            // Search for the lib file
            Optional<Path> libFile = fileStream.filter(p -> p.getFileName().toString().startsWith("jindex")).findFirst();
            if (!libFile.isPresent())
                throw new RuntimeException("Could not find jindex lib");

            // Copy it to the temp directory
            Path tempFilePath = Paths.get(System.getProperty("java.io.tmpdir")).resolve(libFile.get().getFileName().toString());
            if (System.getenv("JINDEX_DEV") != null || !Files.exists(tempFilePath)) {
                Files.copy(ClassIndex.class.getResourceAsStream("/lib/" + libFile.get().getFileName()), tempFilePath, StandardCopyOption.REPLACE_EXISTING);
            }

            System.load(tempFilePath.toAbsolutePath().toString());
        } catch (UnsupportedOperationException ignored) {
            // Closing might be unsupported
        } catch (Exception e) {
            throw new RuntimeException("Unable to load native library", e);
        }
    }

    private boolean destroyed;

    private ClassIndex() {
        super(0, 0);
    }

    public native IndexedClass findClass(String packageName, String className);

    public native IndexedClass[] findClasses(String query, SearchOptions options);

    public native IndexedPackage findPackage(String packageName);

    public native IndexedPackage[] findPackages(String query);

    public List<String> findMethods(String query, int limit) {
        throw new UnsupportedOperationException();
    }

    public native void saveToFile(String filePath);

    public native void destroy();

    private native void createClassIndex(List<byte[]> classes);

    private native void createClassIndexFromJars(List<String> classes);

    private native void loadClassIndexFromFile(String filePath);

    @Override
    protected void finalize() {
        if (this.destroyed)
            return;

        destroy();
    }

    public static ClassIndex fromJars(List<String> jarFileNames) {
        ClassIndex c = new ClassIndex();
        c.createClassIndexFromJars(jarFileNames);
        return c;
    }

    public static ClassIndex fromBytecode(List<byte[]> classes) {
        ClassIndex c = new ClassIndex();
        c.createClassIndex(classes);
        return c;
    }

    public static ClassIndex fromFile(String path) {
        ClassIndex c = new ClassIndex();
        c.loadClassIndexFromFile(path);
        return c;
    }
}
