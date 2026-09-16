package com.github.tth05.jindex;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import org.objectweb.asm.*;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.List;
import static org.junit.jupiter.api.Assertions.*;

class ParameterNamesTest {
    @Test void readsOriginalNamesAndPreservesThemInSnapshots(@TempDir Path directory) throws Exception {
        Path snapshot = directory.resolve("names.index");
        try (ClassIndex index = ClassIndex.fromBytes(List.of(fixture()))) {
            verify(index);
            index.saveToFile(snapshot.toString());
        }
        try (ClassIndex index = ClassIndex.fromFile(snapshot.toString())) { verify(index); }
    }

    private static void verify(ClassIndex index) {
        var methods = index.findClass("Names").getMethods();
        assertArrayEquals(new String[]{"ticks", "ratio", "名前"}, named(methods, "instance").getParameterNames());
        assertArrayEquals(new String[]{"time", "amount", "count"}, named(methods, "statik").getParameterNames());
        assertArrayEquals(new String[]{null, null}, named(methods, "unknown").getParameterNames());
        assertArrayEquals(new String[]{"outer", "value"}, named(methods, "<init>").getParameterNames());
        assertArrayEquals(new String[]{null}, named(methods, "reused").getParameterNames());
    }

    private static IndexedMethod named(IndexedMethod[] methods, String name) {
        return Arrays.stream(methods).filter(method -> method.getName().equals(name)).findFirst().orElseThrow();
    }

    private static byte[] fixture() {
        var writer = new ClassWriter(0);
        writer.visit(Opcodes.V21, Opcodes.ACC_PUBLIC | Opcodes.ACC_ABSTRACT, "Names", null, "java/lang/Object", null);
        var method = writer.visitMethod(Opcodes.ACC_PUBLIC, "instance", "(JDLjava/lang/String;)V", null, null);
        method.visitParameter("ticks", 0);
        method.visitParameter(null, 0);
        method.visitParameter("名前", 0);
        body(method, new String[]{"otherTicks", "ratio", "otherName"}, new String[]{"J", "D", "Ljava/lang/String;"}, new int[]{1,3,5}, 6);
        method = writer.visitMethod(Opcodes.ACC_PUBLIC | Opcodes.ACC_STATIC, "statik", "(JDI)V", null, null);
        body(method, new String[]{"time", "amount", "count"}, new String[]{"J", "D", "I"}, new int[]{0,2,4}, 5);
        writer.visitMethod(Opcodes.ACC_PUBLIC | Opcodes.ACC_ABSTRACT, "unknown", "(ILjava/lang/String;)V", null, null).visitEnd();
        method = writer.visitMethod(Opcodes.ACC_PUBLIC, "<init>", "(Ljava/lang/Object;I)V", null, null);
        method.visitParameter("outer", Opcodes.ACC_SYNTHETIC | Opcodes.ACC_MANDATED);
        method.visitParameter("value", 0);
        body(method, new String[0], new String[0], new int[0], 3);
        method = writer.visitMethod(Opcodes.ACC_PUBLIC | Opcodes.ACC_STATIC, "reused", "(I)V", null, null);
        Label local = new Label(), end = new Label();
        method.visitCode();
        method.visitInsn(Opcodes.ICONST_0);
        method.visitVarInsn(Opcodes.ISTORE, 0);
        method.visitLabel(local);
        method.visitInsn(Opcodes.RETURN);
        method.visitLabel(end);
        method.visitLocalVariable("laterLocal", "I", null, local, end, 0);
        method.visitMaxs(1, 1);
        method.visitEnd();
        writer.visitEnd();
        return writer.toByteArray();
    }

    private static void body(MethodVisitor method, String[] names, String[] types, int[] slots, int locals) {
        Label start = new Label(), end = new Label();
        method.visitCode(); method.visitLabel(start); method.visitInsn(Opcodes.RETURN); method.visitLabel(end);
        for (int i = 0; i < names.length; i++) method.visitLocalVariable(names[i], types[i], null, start, end, slots[i]);
        method.visitMaxs(0, locals); method.visitEnd();
    }
}
