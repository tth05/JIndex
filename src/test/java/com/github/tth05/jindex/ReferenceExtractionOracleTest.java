package com.github.tth05.jindex;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import org.objectweb.asm.ClassReader;
import org.objectweb.asm.ClassVisitor;
import org.objectweb.asm.ClassWriter;
import org.objectweb.asm.ConstantDynamic;
import org.objectweb.asm.FieldVisitor;
import org.objectweb.asm.Handle;
import org.objectweb.asm.MethodVisitor;
import org.objectweb.asm.Opcodes;

import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.EnumSet;
import java.util.HashMap;
import java.util.HashSet;
import java.util.HexFormat;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.*;

/** ASM walks instructions independently of the native parser and its previous-resolver oracle. */
final class ReferenceExtractionOracleTest {
    private record Member(boolean method, String owner, String name, String descriptor) {
        ReferenceTarget target() {
            return method ? ReferenceTarget.methodTarget(owner, name, descriptor)
                    : ReferenceTarget.fieldTarget(owner, name, descriptor);
        }
    }
    private record Site(String owner, String name, String descriptor) { }
    private record Count(long occurrences, Set<ReferenceKind> kinds) { }
    private record Declaration(String parent, List<String> interfaces, Set<Member> members) { }

    @Test
    void memberUsesMatchAsmBeforeAndAfterSnapshotReload(@TempDir Path directory) throws Exception {
        List<byte[]> bytes = new ArrayList<>();
        for (Class<?> fixture : List.of(Calls.class, Parent.class, Child.class, Contract.class)) {
            try (var input = Objects.requireNonNull(fixture.getResourceAsStream("/" + fixture.getName().replace('.', '/') + ".class"))) {
                bytes.add(input.readAllBytes());
            }
        }
        bytes.add(fieldHandles());
        Map<String, Declaration> declarations = declarations(bytes);
        Map<Member, Map<Site, Count>> expected = extract(bytes, declarations);
        assertTrue(expected.size() >= 8, "Fixture lost its field/invocation/handle coverage");
        Path snapshot = directory.resolve("oracle.index");
        try (ClassIndex index = ClassIndex.fromBytes(bytes)) {
            verify(index, declarations, expected);
            index.saveToFile(snapshot.toString());
        }
        try (ClassIndex index = ClassIndex.fromFile(snapshot.toString())) {
            verify(index, declarations, expected);
        }
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        bytes.forEach(digest::update);
        System.out.println("ASM member oracle fixture SHA-256=" + HexFormat.of().formatHex(digest.digest())
                + " classes=" + bytes.size() + " referencedTargets=" + expected.size());
    }

    private static byte[] fieldHandles() {
        ClassWriter writer = new ClassWriter(0);
        writer.visit(Opcodes.V21, Opcodes.ACC_PUBLIC, "oracle/Handles", null, "java/lang/Object", null);
        MethodVisitor method = writer.visitMethod(Opcodes.ACC_PUBLIC | Opcodes.ACC_STATIC, "handles", "()V", null, null);
        method.visitCode();
        String owner = Parent.class.getName().replace('.', '/');
        for (int tag : new int[]{Opcodes.H_GETFIELD, Opcodes.H_PUTFIELD}) {
            method.visitLdcInsn(new Handle(tag, owner, "value", "I", false));
            method.visitInsn(Opcodes.POP);
        }
        method.visitInsn(Opcodes.RETURN);
        method.visitMaxs(1, 0);
        method.visitEnd();
        writer.visitEnd();
        return writer.toByteArray();
    }

    private static Map<String, Declaration> declarations(List<byte[]> bytes) {
        Map<String, Declaration> declarations = new LinkedHashMap<>();
        for (byte[] file : bytes) {
            new ClassReader(file).accept(new ClassVisitor(Opcodes.ASM9) {
                private String owner;
                private Set<Member> members;
                @Override public void visit(int version, int access, String name, String signature, String parent, String[] interfaces) {
                    owner = name;
                    members = new HashSet<>();
                    declarations.put(name, new Declaration(parent, Arrays.asList(interfaces), members));
                }
                @Override public FieldVisitor visitField(int access, String name, String descriptor, String signature, Object value) {
                    members.add(new Member(false, owner, name, descriptor));
                    return null;
                }
                @Override public MethodVisitor visitMethod(int access, String name, String descriptor, String signature, String[] exceptions) {
                    if ((access & Opcodes.ACC_SYNTHETIC) == 0) members.add(new Member(true, owner, name, descriptor));
                    return null;
                }
            }, ClassReader.SKIP_CODE | ClassReader.SKIP_DEBUG | ClassReader.SKIP_FRAMES);
        }
        return declarations;
    }

    private static Map<Member, Map<Site, Count>> extract(List<byte[]> files, Map<String, Declaration> declarations) {
        Map<Member, Map<Site, Count>> references = new HashMap<>();
        for (byte[] file : files) {
            ClassReader reader = new ClassReader(file);
            String source = reader.getClassName();
            reader.accept(new ClassVisitor(Opcodes.ASM9) {
                @Override public MethodVisitor visitMethod(int access, String name, String descriptor, String signature, String[] exceptions) {
                    Site site = new Site(source, name, descriptor);
                    return new MethodVisitor(Opcodes.ASM9) {
                        private void add(boolean method, String owner, String name, String descriptor, ReferenceKind kind) {
                            Member target = resolve(new Member(method, owner, name, descriptor), declarations, new HashSet<>());
                            if (target == null) return;
                            references.computeIfAbsent(target, ignored -> new HashMap<>()).merge(site,
                                    new Count(1, EnumSet.of(kind)), (first, next) -> {
                                        Set<ReferenceKind> kinds = EnumSet.copyOf(first.kinds());
                                        kinds.addAll(next.kinds());
                                        return new Count(first.occurrences() + next.occurrences(), kinds);
                                    });
                        }
                        @Override public void visitFieldInsn(int opcode, String owner, String name, String descriptor) {
                            add(false, owner, name, descriptor, opcode == Opcodes.PUTFIELD || opcode == Opcodes.PUTSTATIC
                                    ? ReferenceKind.FIELD_WRITE : ReferenceKind.FIELD_READ);
                        }
                        @Override public void visitMethodInsn(int opcode, String owner, String name, String descriptor, boolean isInterface) {
                            add(true, owner, name, descriptor, ReferenceKind.METHOD_INVOKE);
                        }
                        private void constant(Object value) {
                            if (value instanceof Handle handle) {
                                boolean field = handle.getTag() <= Opcodes.H_PUTSTATIC;
                                add(!field, handle.getOwner(), handle.getName(), handle.getDesc(),
                                        field ? ReferenceKind.FIELD_HANDLE : ReferenceKind.METHOD_HANDLE);
                            } else if (value instanceof ConstantDynamic dynamic) {
                                constant(dynamic.getBootstrapMethod());
                                for (int index = 0; index < dynamic.getBootstrapMethodArgumentCount(); index++) constant(dynamic.getBootstrapMethodArgument(index));
                            }
                        }
                        @Override public void visitLdcInsn(Object value) { constant(value); }
                        @Override public void visitInvokeDynamicInsn(String name, String descriptor, Handle bootstrap, Object... arguments) {
                            constant(bootstrap);
                            for (Object argument : arguments) constant(argument);
                        }
                    };
                }
            }, ClassReader.SKIP_DEBUG | ClassReader.SKIP_FRAMES);
        }
        return references;
    }

    private static Member resolve(Member member, Map<String, Declaration> declarations, Set<String> seen) {
        if (!seen.add(member.owner())) return null;
        Declaration owner = declarations.get(member.owner());
        if (owner == null) return null;
        if (owner.members().contains(member)) return member;
        if (member.name().equals("<init>")) return null;
        List<String> parents = new ArrayList<>(owner.interfaces());
        if (owner.parent() != null) parents.add(member.method() ? 0 : parents.size(), owner.parent());
        for (String parent : parents) {
            Member resolved = resolve(new Member(member.method(), parent, member.name(), member.descriptor()), declarations, seen);
            if (resolved != null) return resolved;
        }
        return null;
    }

    private static void verify(ClassIndex index, Map<String, Declaration> declarations, Map<Member, Map<Site, Count>> expected) {
        for (Declaration declaration : declarations.values()) for (Member member : declaration.members()) {
            ReferenceSearchPage page = index.findReferences(member.target(), 100_000);
            assertFalse(page.truncated(), member.toString());
            Map<Site, Count> actual = new HashMap<>();
            for (ReferenceResult result : page.results()) {
                assertEquals(ReferenceSiteKind.METHOD, result.kind());
                assertNull(actual.put(new Site(result.ownerInternalName(), result.name(), result.descriptor()),
                        new Count(result.occurrenceCount(), result.kinds())), "Duplicate result site");
            }
            assertEquals(expected.getOrDefault(member, Map.of()), actual, member.toString());
        }
    }

    interface Contract { void run(); }
    static class Parent {
        int value;
        static int shared;
        static void task() { }
        void work() { }
    }
    static final class Child extends Parent implements Contract {
        @Override public void run() { work(); }
    }
    static final class Calls {
        void inspect(Child child, Contract contract) {
            child.value++;
            child.value += Parent.shared;
            Parent.shared = child.value;
            child.work();
            child.work();
            Parent.task();
            contract.run();
            child.run();
            Runnable reference = child::work;
            java.util.function.Supplier<Child> constructor = Child::new;
            reference.run();
            constructor.get();
        }
    }
}
