package com.github.tth05.jindex;

import java.io.OutputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Arrays;

public class NativeTest {

    static {
        try {
            Path tempFilePath = Paths.get(System.getProperty("java.io.tmpdir"), "jindex_lib.dll");
            Path inputPath = Paths.get(NativeTest.class.getResource("/jindex_rs.dll").toURI());

            OutputStream outputStream = Files.newOutputStream(tempFilePath);
            outputStream.write(Files.readAllBytes(inputPath));
            outputStream.close();

            System.load(tempFilePath.toAbsolutePath().toString());
        } catch (Exception e) {
            e.printStackTrace();
        }
    }

    public static native long createClassIndex();

    public static native FindClassesResult[] findClasses(long classIndexPointer, String query, int limit);

    public static void main(String[] args) {
        double t = System.nanoTime();
        long classIndex = createClassIndex();
        System.out.println((System.nanoTime() - t) / 1_000_000d);

        t = System.nanoTime();
        FindClassesResult[] results = findClasses(classIndex, "Ab", 50);
        t = (System.nanoTime() - t) / 1_000_000d;
        System.out.println(Arrays.toString(results));
        System.out.println(results.length + " - " + t);
//        System.out.println(Arrays.toString(findClasses(classIndex, "ClassIndex")));
    }
}
