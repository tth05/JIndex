package com.github.tth05.jindex;

import io.github.classgraph.ClassGraph;
import io.github.classgraph.ClassInfo;
import io.github.classgraph.ScanResult;
import org.junit.jupiter.api.Test;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.net.URL;
import java.security.CodeSource;
import java.security.ProtectionDomain;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

public class BasicTest {

    @Test
    public void test() throws Throwable {
        List<byte[]> classes = new ArrayList<>();
        try (ScanResult r = new ClassGraph().enableSystemJarsAndModules().scan()) {
            System.out.println(r.getAllClasses().size());
            for (ClassInfo c : r.getAllClasses()) {
                try {
                    classes.add(getBytecode(c.loadClass()));
                } catch (Throwable ignored) {
                }
            }
        }

        double t = System.nanoTime();
        ClassIndex index = new ClassIndex(classes);
        System.out.println((System.nanoTime() - t) / 1_000_000d);

        t = System.nanoTime();
        FindClassesResult[] results = index.findClasses("Array", 50);
        t = (System.nanoTime() - t) / 1_000_000d;
        System.out.println(Arrays.toString(results));
        System.out.println(results.length + " - " + t);
//        System.out.println(Arrays.toString(findClasses(classIndex, "ClassIndex")));
    }

    public static byte[] getBytecode(Class<?> clazz) {
        String codeSource = getClassCodeSourceName(clazz);
        if (codeSource == null)
            return null;

        InputStream inputStream = null;

        if (clazz.getClassLoader() != null)
            inputStream = clazz.getClassLoader().getResourceAsStream(codeSource);

        if (inputStream == null) {
            inputStream = ClassLoader.getSystemResourceAsStream(clazz.getName().replace('.', '/') + ".class");
            if (inputStream == null)
                throw new RuntimeException("Class " + clazz.getName() + " not found");
        }

        try {
            byte[] buffer = new byte[8192];
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            int r;
            while ((r = inputStream.read(buffer, 0, buffer.length)) != -1) {
                out.write(buffer, 0, r);
            }

            return out.toByteArray();
        } catch (IOException e) {
            return null;
        } finally {
            try {
                inputStream.close();
            } catch (IOException e) {
            }
        }
    }

    public static String getClassCodeSourceName(Class<?> clazz) {
        ProtectionDomain protectionDomain = clazz.getProtectionDomain();
        if (protectionDomain == null)
            return getClassCodeSourceNameUsingClassLoader(clazz);

        CodeSource codeSourceObj = protectionDomain.getCodeSource();
        if (codeSourceObj == null || codeSourceObj.getLocation() == null)
            return getClassCodeSourceNameUsingClassLoader(clazz);

        String codeSource = codeSourceObj.getLocation().toString();
        if (codeSource.startsWith("jar"))
            codeSource = codeSource.substring(codeSource.lastIndexOf('!') + 2);
        else
            codeSource = clazz.getName().replace(".", "/") + ".class";
        return codeSource;
    }

    private static String getClassCodeSourceNameUsingClassLoader(Class<?> clazz) {
        URL resource = ClassLoader.getSystemResource(clazz.getName().replace('.', '/') + ".class");
        if (resource == null)
            return null;

        String resourcePath = resource.toString();
        if (!resourcePath.startsWith("jar"))
            throw new IllegalStateException("Resource not from jar");

        return resourcePath.substring(resourcePath.lastIndexOf('!') + 2);
    }
}
