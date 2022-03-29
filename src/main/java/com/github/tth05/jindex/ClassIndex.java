package com.github.tth05.jindex;

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.nio.file.StandardCopyOption;
import java.util.List;

public class ClassIndex {

    static {
        try {
            boolean isDev = false;
            Path tempFilePath = Paths.get(System.getProperty("java.io.tmpdir")).resolve("jindex_lib_0.0.14.dll");
            if (isDev || !Files.exists(tempFilePath)) {
                Files.copy(ClassIndex.class.getResourceAsStream("/jindex_rs.dll"), tempFilePath, StandardCopyOption.REPLACE_EXISTING);
            }

            System.load(tempFilePath.toAbsolutePath().toString());
        } catch (Exception e) {
            throw new RuntimeException("Unable to load native library", e);
        }
    }

    private long classIndexPointer;
    private boolean destroyed;

    public native IndexedClass[] findClasses(String query, int limit);

    public native IndexedClass findClass(String packageName, String className);

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
