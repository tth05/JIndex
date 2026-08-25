package com.github.tth05.jindex;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.io.InputStream;
import java.util.List;
import java.util.Objects;

import static org.junit.jupiter.api.Assertions.assertEquals;

final class CodeInsightSummaryTest {

    @Test
    void summarizesReferencesAndHierarchyWithoutMaterializingResults() throws Exception {
        try (ClassIndex index = ClassIndex.fromBytes(List.of(
                classBytes(InsightBase.class),
                classBytes(InsightOne.class),
                classBytes(InsightTwo.class)
        ))) {
            String owner = internalName(InsightBase.class);
            ReferenceSummary references = index.summarizeReferences(
                    ReferenceTarget.methodTarget(owner, "run", "()V")
            );
            assertEquals(1, references.siteCount());
            assertEquals(2, references.occurrenceCount());

            IndexedClass base = find(index, InsightBase.class);
            HierarchySummary baseSummary = base.summarizeHierarchy();
            assertEquals(2, baseSummary.implementationCount());
            assertEquals(base.getMethods().length, baseSummary.methodImplementationCounts().length);
            assertEquals(2, countFor(base, baseSummary.methodImplementationCounts(), "run", "()V"));
            assertEquals(0, countFor(base, baseSummary.methodBaseCounts(), "run", "()V"));

            IndexedClass one = find(index, InsightOne.class);
            HierarchySummary oneSummary = one.summarizeHierarchy();
            assertEquals(1, oneSummary.implementationCount());
            assertEquals(1, countFor(one, oneSummary.methodImplementationCounts(), "run", "()V"));
            assertEquals(1, countFor(one, oneSummary.methodBaseCounts(), "run", "()V"));
            assertEquals(0, countFor(one, oneSummary.methodImplementationCounts(), "<init>", "()V"));
            assertEquals(0, countFor(one, oneSummary.methodImplementationCounts(), "staticTask", "()V"));
        }
    }

    @Test
    void missingClassHasAnEmptyReferenceSummary() {
        try (ClassIndex index = ClassIndex.fromBytes(List.of(classBytesUnchecked(InsightBase.class)))) {
            assertEquals(
                    new ReferenceSummary(0, 0),
                    index.summarizeReferences(ReferenceTarget.classTarget("missing/Type"))
            );
        }
    }

    private static int countFor(IndexedClass owner, int[] counts, String name, String descriptor) {
        IndexedMethod[] methods = owner.getMethods();
        for (int index = 0; index < methods.length; index++) {
            if (methods[index].getName().equals(name)
                    && methods[index].getDescriptorString().equals(descriptor)) {
                return counts[index];
            }
        }
        throw new AssertionError("Method not found: " + name + descriptor);
    }

    private static IndexedClass find(ClassIndex index, Class<?> type) {
        String internalName = internalName(type);
        int separator = internalName.lastIndexOf('/');
        return Objects.requireNonNull(index.findClass(
                internalName.substring(0, separator),
                internalName.substring(separator + 1)
        ));
    }

    private static String internalName(Class<?> type) {
        return type.getName().replace('.', '/');
    }

    private static byte[] classBytes(Class<?> type) throws IOException {
        String resource = '/' + internalName(type) + ".class";
        try (InputStream input = Objects.requireNonNull(type.getResourceAsStream(resource), resource)) {
            return input.readAllBytes();
        }
    }

    private static byte[] classBytesUnchecked(Class<?> type) {
        try {
            return classBytes(type);
        } catch (IOException exception) {
            throw new AssertionError(exception);
        }
    }

    private interface InsightBase {
        void run();
    }

    private static class InsightOne implements InsightBase {
        @Override
        public void run() {
        }

        void invokeTwice(InsightBase target) {
            target.run();
            target.run();
        }

        static void staticTask() {
        }
    }

    private static final class InsightTwo extends InsightOne {
        @Override
        public void run() {
        }

        static void staticTask() {
        }
    }
}
