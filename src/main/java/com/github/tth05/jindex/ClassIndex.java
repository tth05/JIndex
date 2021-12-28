package com.github.tth05.jindex;

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.List;

public class ClassIndex {

    static {
        try {
            Path tempFilePath = Files.createTempFile("jindex_lib", ".dll");
            Files.copy(ClassIndex.class.getResourceAsStream("/jindex_rs.dll"), tempFilePath, StandardCopyOption.REPLACE_EXISTING);
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

    private native void createClassIndex(List<byte[]> classes);

    private native void loadClassIndexFromFile(String filePath);
}
