package com.github.tth05.jindex;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.time.Duration;
import java.util.Arrays;
import java.util.List;
import java.util.Objects;

import static org.junit.jupiter.api.Assertions.*;

final class RegressionTest {
    @Test
    void unicodeGenericSignaturesFallBackToErasedDeclarations() throws IOException {
        try (ClassIndex index = indexOf(UnicodeClass.class, UnicodeField.class, UnicodeMethod.class)) {
            IndexedClass type = Objects.requireNonNull(index.findClass(UnicodeClass.class.getName()));
            assertNull(type.getGenericSignatureString());

            IndexedField field = Objects.requireNonNull(index.findClass(UnicodeField.class.getName())).getFields()[0];
            assertEquals("field", field.getName());
            assertNull(field.getGenericSignatureString());
            assertEquals("Ljava/lang/Object;", field.getDescriptorString());

            IndexedMethod method = Arrays.stream(Objects.requireNonNull(index.findClass(UnicodeMethod.class.getName())).getMethods())
                    .filter(candidate -> candidate.getName().equals("run")).findFirst().orElseThrow();
            assertNull(method.getGenericSignatureString());
            assertEquals("(Ljava/lang/Object;)V", method.getDescriptorString());
        }
    }

    @Test
    void unicodeClassErasurePreservesMemberTypesAndExceptions(@TempDir Path directory) throws IOException {
        Path snapshot = directory.resolve("unicode.index");
        try (ClassIndex index = indexOf(MixedUnicodeClass.class, PresentException.class)) {
            assertErasedUnicodeMembers(index);
            index.saveToFile(snapshot.toString());
        }
        try (ClassIndex index = ClassIndex.fromFile(snapshot.toString())) {
            assertErasedUnicodeMembers(index);
        }
    }

    private static void assertErasedUnicodeMembers(ClassIndex index) {
        IndexedClass type = Objects.requireNonNull(index.findClass(MixedUnicodeClass.class.getName()));
        String exceptionDescriptor = "L" + PresentException.class.getName().replace('.', '/') + ";";
        assertNull(type.getGenericSignatureString());
        assertNull(type.getFields()[0].getGenericSignatureString());
        assertEquals(exceptionDescriptor, type.getFields()[0].getDescriptorString());
        IndexedMethod method = Arrays.stream(type.getMethods())
                .filter(candidate -> candidate.getName().equals("run")).findFirst().orElseThrow();
        assertNull(method.getGenericSignatureString());
        assertEquals("(" + exceptionDescriptor + ")" + exceptionDescriptor, method.getDescriptorString());
        assertEquals(List.of(PresentException.class.getName()),
                Arrays.stream(method.getExceptions()).map(IndexedClass::getNameWithPackageDot).toList());
    }

    @Test
    void descriptorStringsAreTheExactJvmDescriptors() throws IOException {
        try (ClassIndex index = indexOf(ThrowsFixture.class)) {
            IndexedClass type = Objects.requireNonNull(index.findClass(ThrowsFixture.class.getName()));
            IndexedMethod method = Arrays.stream(type.getMethods())
                    .filter(candidate -> candidate.getName().equals("unresolvedParameter")).findFirst().orElseThrow();
            assertEquals("(Ljava/util/List;Lcom/github/tth05/jindex/CycleA;)Ljava/util/Map;", method.getDescriptorString());
            assertEquals("Ljava/util/List;", type.getFields()[0].getDescriptorString());
        }
    }

    @Test
    void cyclicHierarchiesTerminateWithoutIncludingTheTargetItself() throws IOException {
        byte[] cycle = classBytes(CycleA.class);
        replaceSuperclass(cycle, CycleB.class.getName().replace('.', '/'));
        try (ClassIndex index = ClassIndex.fromBytes(List.of(cycle, classBytes(CycleB.class), classBytes(Needle.class)))) {
            assertTimeout(Duration.ofSeconds(2), () -> {
                IndexedClass target = Objects.requireNonNull(index.findClass(CycleA.class.getName()));
                assertEquals(List.of(CycleB.class.getName()), Arrays.stream(target.findImplementations(false))
                        .map(IndexedClass::getNameWithPackageDot).toList());
                assertEquals(0, Objects.requireNonNull(index.findClass(Needle.class.getName())).findImplementations(false).length);
                IndexedMethod method = Arrays.stream(target.getMethods()).filter(candidate -> candidate.getName().equals("run")).findFirst().orElseThrow();
                assertEquals(1, method.findBaseMethods().length);
            });
        }
    }

    private static void replaceSuperclass(byte[] bytes, String replacement) {
        ByteBuffer data = ByteBuffer.wrap(bytes);
        data.position(8);
        int count = Short.toUnsignedInt(data.getShort());
        String[] strings = new String[count];
        int[] classes = new int[count];
        for (int index = 1; index < count; index++) {
            int tag = Byte.toUnsignedInt(data.get());
            switch (tag) {
                case 1 -> {
                    byte[] value = new byte[Short.toUnsignedInt(data.getShort())];
                    data.get(value);
                    strings[index] = new String(value, StandardCharsets.UTF_8);
                }
                case 7 -> classes[index] = Short.toUnsignedInt(data.getShort());
                case 3, 4, 9, 10, 11, 12, 17, 18 -> data.position(data.position() + 4);
                case 5, 6 -> { data.position(data.position() + 8); index++; }
                case 8, 16, 19, 20 -> data.position(data.position() + 2);
                case 15 -> data.position(data.position() + 3);
                default -> throw new AssertionError("Unexpected constant pool tag " + tag);
            }
        }
        for (int index = 1; index < count; index++) {
            if (replacement.equals(strings[classes[index]])) {
                data.putShort(data.position() + 4, (short) index);
                return;
            }
        }
        throw new AssertionError("Superclass fixture entry missing: " + replacement);
    }
    @Test
    void aLongerPrefixReturnsNoMatches() throws IOException {
        try (ClassIndex index = indexOf(Needle.class)) {
            assertEquals(0, index.findClasses("Needles", SearchOptions.defaultOptions()).results().length);
        }
    }

    @Test
    void hierarchyQueriesDoNotLeakAncestorsBetweenCandidates() throws IOException {
        try (ClassIndex index = indexOf(A.class, B.class, C.class, I.class, J.class)) {
            IndexedClass target = Objects.requireNonNull(index.findClass(I.class.getName()));
            assertEquals(List.of(A.class.getName(), B.class.getName(), J.class.getName()),
                    Arrays.stream(target.findImplementations(false)).map(IndexedClass::getNameWithPackageDot).toList());
            assertEquals(3, target.summarizeHierarchy().implementationCount());
        }
    }

    @Test
    void exceptionsContainOnlyResolvedClasses() throws IOException {
        try (ClassIndex index = indexOf(ThrowsFixture.class, PresentException.class)) {
            IndexedMethod method = Arrays.stream(Objects.requireNonNull(index.findClass(ThrowsFixture.class.getName())).getMethods())
                    .filter(candidate -> candidate.getName().equals("run")).findFirst().orElseThrow();
            assertEquals(List.of(PresentException.class.getName()),
                    Arrays.stream(method.getExceptions()).map(IndexedClass::getNameWithPackageDot).toList());
        }
    }

    @Test
    void exactClassLookupValidatesBothNames() throws IOException {
        try (ClassIndex index = indexOf(Needle.class)) {
            assertThrows(NullPointerException.class, () -> index.findClass(null, "Needle"));
            assertThrows(NullPointerException.class, () -> index.findClass("", null));
        }
    }

    private static ClassIndex indexOf(Class<?>... types) throws IOException {
        var bytes = new java.util.ArrayList<byte[]>();
        for (Class<?> type : types) {
            bytes.add(classBytes(type));
        }
        return ClassIndex.fromBytes(bytes);
    }

    private static byte[] classBytes(Class<?> type) throws IOException {
        try (var input = Objects.requireNonNull(type.getResourceAsStream('/' + type.getName().replace('.', '/') + ".class"))) {
            return input.readAllBytes();
        }
    }

    private interface I {}
    private static class A implements I {}
    private interface J extends I {}
    private static class B extends A implements J {}
    private static class C {}
    private static class MissingException extends Exception { private static final long serialVersionUID = 1L; }
    private static class PresentException extends Exception { private static final long serialVersionUID = 1L; }
    private static class ThrowsFixture<T extends PresentException> {
        java.util.List<T> unresolvedField;
        void run() throws MissingException, T {}
        java.util.Map<String, T> unresolvedParameter(java.util.List<T> values, CycleA cycle) { return null; }
    }
    private static class UnicodeClass<Ä> {}
    private static class UnicodeField<Ä> { Ä field; }
    private static class UnicodeMethod { <Ä> void run(Ä value) {} }
    private static class MixedUnicodeClass<Ä, T extends PresentException> {
        T field;
        T run(T value) throws T { return value; }
    }
}

final class Needle {}
class CycleA {
    Object create() { return new CycleB(); }
    public void run() {}
}
class CycleB extends CycleA {
    @Override public void run() {}
}
