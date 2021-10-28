package com.github.tth05.jindex;

import java.io.OutputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.List;

public class ClassIndex {

    static {
        loadNativeLibrary();
    }

    private final long pointer;

    public ClassIndex(List<byte[]> classes) {
        this.pointer = createClassIndex(classes);
    }

    public FindClassesResult[] findClasses(String query, int limit) {
        return findClasses(this.pointer, query, limit);
    }

    public List<String> findMethods(String query) {
        throw new UnsupportedOperationException();
    }

    private static native FindClassesResult[] findClasses(long classIndexPointer, String query, int limit);

    public static native long createClassIndex(List<byte[]> classes);

    static void loadNativeLibrary() {
        try {
            Path tempFilePath = Paths.get(System.getProperty("java.io.tmpdir"), "jindex_lib.dll");
            Path inputPath = Paths.get(ClassIndex.class.getResource("/jindex_rs.dll").toURI());

            OutputStream outputStream = Files.newOutputStream(tempFilePath);
            outputStream.write(Files.readAllBytes(inputPath));
            outputStream.close();

            System.load(tempFilePath.toAbsolutePath().toString());
        } catch (Exception e) {
            throw new RuntimeException("Unable to load native library", e);
        }
    }
}
