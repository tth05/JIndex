package com.github.tth05.jindex.tests;

import com.github.tth05.jindex.ClassIndex;
import com.github.tth05.jindex.constantPool.IndexString;
import com.github.tth05.jindex.entries.IndexedClass;
import io.github.classgraph.ClassGraph;
import io.github.classgraph.ScanResult;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.*;
import java.util.stream.Collectors;

public class BasicTest {

    @Test
    public void test() {
        ClassIndex theIndex = doTheTest();
        try {
            System.gc();
            Thread.sleep(10000);
        } catch (InterruptedException e) {
            e.printStackTrace();
        }
        System.out.println(theIndex);
    }

    public ClassIndex doTheTest() {
        //        List<Class<?>> allClasses = getTestClasses();
        long t = System.nanoTime();
        Map.Entry<String, IndexString[]>[] entries = null;
        try {
            //noinspection unchecked
            entries = Files.readAllLines(Paths.get(".", "classes1.txt"))
                    .stream().collect(HashMap::new, (map, s) -> {
                        String[] ar = s.split(";");
                        map.put(ar[0], ar.length == 1 ? new IndexString[0] : Arrays.stream(Arrays.copyOfRange(ar, 1, ar.length))
                                .map(IndexString::new).toArray(IndexString[]::new));
                    }, HashMap::putAll).entrySet().toArray(new Map.Entry[0]);
        } catch (IOException e) {
            e.printStackTrace();
            return null;
        }

        System.out.printf("Loaded classes in %.2fms%n", (System.nanoTime() - t) / 1_000_000d);
        System.out.println("Method count: " + Arrays.stream(entries).mapToLong(e -> e.getValue().length).sum());
        System.out.println("Class count: " + entries.length);

        Map.Entry<String, IndexString[]>[] finalEntries = entries;
        ClassIndex classIndex = doWarmup(finalEntries);
        t = System.nanoTime();
        classIndex = new ClassIndex.Builder(new ClassIndex.Builder.IClassInfoIterator() {

            private int index;
            private IndexString currentClassName = new IndexString(finalEntries[this.index].getKey());

            @Override
            public boolean hasNext() {
                return index < finalEntries.length;
            }

            @Override
            public void advance() {
                this.index++;
                if (hasNext())
                    this.currentClassName = new IndexString(finalEntries[this.index].getKey());
            }

            @Override
            public int elementCount() {
                return finalEntries.length;
            }

            @Override
            public IndexString currentPackageName() {
                return this.currentClassName.subSequenceBeforeLast((byte) '.');
            }

            @Override
            public IndexString currentClassName() {
                return this.currentClassName.subSequenceAfterLast((byte) '.');
            }

            @Override
            public List<IndexString> currentMethodNames() {
                return Arrays.asList(finalEntries[this.index].getValue());
            }
        }).setExpectedMethodCount(Arrays.stream(entries).mapToInt(e -> e.getValue().length).sum()).build();
        System.out.println("Constructed index in: " + ((System.nanoTime() - t) / 1_000_000d) + "ms");
        t = System.nanoTime();
        List<IndexedClass> result = classIndex.findClasses(new IndexString("Intrinsics"));
        ClassIndex finalClassIndex = classIndex;
        System.out.printf(
                "%.4fms -> %d -> %s%n",
                (System.nanoTime() - t) / 1_000_000d,
                result.size(),
                result.stream().map(c -> c.getFullClassName(finalClassIndex.getConstantPool())).collect(Collectors.toList())
        );

        t = System.nanoTime();
        List<String> result2 = classIndex.findMethods(new IndexString("findClass"));
        System.out.printf(
                "%.4fms -> %d -> %s%n",
                (System.nanoTime() - t) / 1_000_000d,
                result2.size(),
                result2
        );
        return classIndex;
    }

    public ClassIndex doWarmup(Map.Entry<String, IndexString[]>[] finalEntries) {
        return new ClassIndex.Builder(new ClassIndex.Builder.IClassInfoIterator() {

            private int index;
            private IndexString currentClassName = new IndexString(finalEntries[this.index].getKey());

            @Override
            public boolean hasNext() {
                return index < finalEntries.length;
            }

            @Override
            public void advance() {
                this.index++;
                if (hasNext())
                    this.currentClassName = new IndexString(finalEntries[this.index].getKey());
            }

            @Override
            public int elementCount() {
                return finalEntries.length;
            }

            @Override
            public IndexString currentPackageName() {
                return this.currentClassName.subSequenceBeforeLast((byte) '.');
            }

            @Override
            public IndexString currentClassName() {
                return this.currentClassName.subSequenceAfterLast((byte) '.');
            }

            @Override
            public List<IndexString> currentMethodNames() {
                return Arrays.asList(finalEntries[this.index].getValue());
            }
        }).setExpectedMethodCount(Arrays.stream(finalEntries).mapToInt(e -> e.getValue().length).sum()).build();
    }

    public List<Class<?>> getTestClasses() {
        try (ScanResult result = new ClassGraph().enableSystemJarsAndModules().enableClassInfo().scan()) {
            return result.getAllClasses().stream().map((ci) -> {
                try {
                    return ci.loadClass();
                } catch (Throwable ignored) {
                    return null;
                }
            }).filter(Objects::nonNull).filter(c -> c.getPackage() != null).collect(Collectors.toList());
        }
    }
}
