package com.github.tth05.jindex;

import io.github.classgraph.ClassGraph;
import io.github.classgraph.ScanResult;

import java.io.DataInputStream;
import java.io.IOException;
import java.net.URL;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Enumeration;
import java.util.List;
import java.util.Objects;
import java.util.stream.Collectors;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;
import java.util.zip.ZipOutputStream;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;

public class SampleClassesHelper {

    private static final boolean USE_SAMPLE_JAR_CACHE = true;

    public static List<byte[]> loadSampleClasses() {
        if (!USE_SAMPLE_JAR_CACHE)
            return readSampleClasses();

        createSamplesJar();

        List<byte[]> classes = new ArrayList<>();

        try (ZipFile zipFile = new ZipFile("src/test/resources/Samples.jar")) {
            for (Enumeration<? extends ZipEntry> iter = zipFile.entries(); iter.hasMoreElements(); ) {
                ZipEntry el = iter.nextElement();

                byte[] buf = new byte[(int) el.getSize()];
                new DataInputStream(zipFile.getInputStream(el)).readFully(buf);
                classes.add(buf);
            }

            return classes;
        } catch (IOException e) {
            e.printStackTrace();
        }

        return null;
    }

    public static void createSamplesJar() {
        URL resource = SampleClassesHelper.class.getResource("/Samples.jar");
        if (resource == null) {
            try (ZipOutputStream zipFile = new ZipOutputStream(Files.newOutputStream(Paths.get("src/test/resources/Samples.jar")))) {
                int i = 0;
                for (byte[] b : readSampleClasses()) {
                    zipFile.putNextEntry(new ZipEntry("Class" + i + ".class"));
                    zipFile.write(b);
                    zipFile.closeEntry();
                    i++;
                }
            } catch (IOException e) {
                e.printStackTrace();
            }
        }
    }

    private static List<byte[]> readSampleClasses() {
        return assertDoesNotThrow(() -> {
            try (ScanResult result = new ClassGraph().enableSystemJarsAndModules().disableRuntimeInvisibleAnnotations().scan()) {
                return result.getAllClasses().stream().map(c -> assertDoesNotThrow(() -> c.getResource().load())).filter(Objects::nonNull).collect(Collectors.toList());
            }
        });
    }
}
