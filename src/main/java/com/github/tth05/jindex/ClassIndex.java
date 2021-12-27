package com.github.tth05.jindex;

import java.io.OutputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.List;

public class ClassIndex {

    static {
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

    private long pointer;

    public ClassIndex(String filePath) {
        loadClassIndexFromFile(filePath);
    }

    public ClassIndex(List<byte[]> classes) {
        createClassIndex(classes);
    }

    public native IndexedClass[] findClasses(String query, int limit);

    public List<String> findMethods(String query, int limit) {
        throw new UnsupportedOperationException();
    }

    public native void saveToFile(String filePath);

    private native long createClassIndex(List<byte[]> classes);

    private native long loadClassIndexFromFile(String filePath);
}
