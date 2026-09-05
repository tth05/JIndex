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
    record Member(boolean method, String owner, String name, String descriptor) {
        ReferenceTarget target() {
            return method ? ReferenceTarget.methodTarget(owner, name, descriptor)
                    : ReferenceTarget.fieldTarget(owner, name, descriptor);
        }
    }
    record Site(String owner, String name, String descriptor) { }
    record Count(long occurrences, Set<ReferenceKind> kinds) { }
    record Declaration(String parent, List<String> interfaces, Set<Member> members, Map<Member, Integer> flags) { }

    @Test
    void memberUsesMatchAsmBeforeAndAfterSnapshotReload(@TempDir Path directory) throws Exception {
        assertEquals(2, new ChildCarrier().ping(), "The compiled fixture selects the more specific default");
        List<byte[]> bytes = new ArrayList<>();
        for (Class<?> fixture : List.of(Calls.class, Parent.class, Child.class, Contract.class, BaseContract.class, ChildContract.class, BaseCarrier.class, ChildCarrier.class)) {
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

    static Map<String, Declaration> declarations(List<byte[]> bytes) {
        Map<String, Declaration> declarations = new LinkedHashMap<>();
        for (byte[] file : bytes) {
            new ClassReader(file).accept(new ClassVisitor(Opcodes.ASM9) {
                private String owner;
                private Set<Member> members;
                private Map<Member, Integer> flags;
                @Override public void visit(int version, int access, String name, String signature, String parent, String[] interfaces) {
                    owner = name;
                    members = new HashSet<>();
                    flags = new HashMap<>();
                    declarations.put(name, new Declaration(parent, Arrays.asList(interfaces), members, flags));
                }
                @Override public FieldVisitor visitField(int access, String name, String descriptor, String signature, Object value) {
                    Member member = new Member(false, owner, name, descriptor);
                    members.add(member);
                    flags.put(member, access);
                    return null;
                }
                @Override public MethodVisitor visitMethod(int access, String name, String descriptor, String signature, String[] exceptions) {
                    if ((access & Opcodes.ACC_SYNTHETIC) == 0) {
                        Member member = new Member(true, owner, name, descriptor);
                        members.add(member);
                        flags.put(member, access);
                    }
                    return null;
                }
            }, ClassReader.SKIP_CODE | ClassReader.SKIP_DEBUG | ClassReader.SKIP_FRAMES);
        }
        return declarations;
    }

    private static Map<Member, Map<Site, Count>> extract(List<byte[]> files, Map<String, Declaration> declarations) {
        Set<Member> targets = new HashSet<>();
        declarations.values().forEach(value -> targets.addAll(value.members()));
        return extract(files, declarations, targets);
    }

    private record Signature(boolean method, String name, String descriptor) { }

    static Map<Member, Map<Site, Count>> extract(List<byte[]> files, Map<String, Declaration> declarations,
                                               Set<Member> targets) {
        Set<Signature> signatures = new HashSet<>();
        targets.forEach(target -> signatures.add(new Signature(target.method(), target.name(), target.descriptor())));
        Map<Member, Map<Site, Count>> references = new HashMap<>();
        for (byte[] file : files) {
            ClassReader reader = new ClassReader(file);
            String source = reader.getClassName();
            reader.accept(new ClassVisitor(Opcodes.ASM9) {
                @Override public MethodVisitor visitMethod(int access, String name, String descriptor, String signature, String[] exceptions) {
                    Site site = new Site(source, name, descriptor);
                    return new MethodVisitor(Opcodes.ASM9) {
                        private void add(boolean method, String owner, String name, String descriptor, ReferenceKind kind) {
                            if (!signatures.contains(new Signature(method, name, descriptor))) return;
                            for (Member target : resolve(new Member(method, owner, name, descriptor), declarations)) {
                                if (!targets.contains(target)) continue;
                                references.computeIfAbsent(target, ignored -> new HashMap<>()).merge(site,
                                    new Count(1, EnumSet.of(kind)), (first, next) -> {
                                        Set<ReferenceKind> kinds = EnumSet.copyOf(first.kinds());
                                        kinds.addAll(next.kinds());
                                        return new Count(first.occurrences() + next.occurrences(), kinds);
                                    });
                            }
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

    private static Set<Member> resolve(Member member, Map<String, Declaration> declarations) {
        if (!member.method()) {
            Member field = resolveField(member, declarations, new HashSet<>());
            return field == null ? Set.of() : Set.of(field);
        }
        Set<String> interfaces = new HashSet<>();
        Set<String> ancestors = new HashSet<>();
        String owner = member.owner();
        while (owner != null && ancestors.add(owner)) {
            Declaration declaration = declarations.get(owner);
            if (declaration == null) break;
            Member candidate = new Member(true, owner, member.name(), member.descriptor());
            if (declaration.members().contains(candidate)) return Set.of(candidate);
            if (member.name().equals("<init>")) return Set.of();
            interfaces.addAll(declaration.interfaces());
            owner = declaration.parent();
        }
        Set<Member> candidates = new HashSet<>();
        for (String name : interfaceClosure(interfaces, declarations)) {
            Declaration declaration = declarations.get(name);
            if (declaration == null) continue;
            Member candidate = new Member(true, name, member.name(), member.descriptor());
            Integer flags = declaration.flags().get(candidate);
            if (flags != null && (flags & (Opcodes.ACC_PRIVATE | Opcodes.ACC_STATIC)) == 0) candidates.add(candidate);
        }
        Set<Member> maximal = new HashSet<>(candidates);
        for (Member candidate : candidates) {
            Declaration declaration = declarations.get(candidate.owner());
            Set<String> supers = interfaceClosure(new HashSet<>(declaration.interfaces()), declarations);
            maximal.removeIf(other -> supers.contains(other.owner()));
        }
        List<Member> defaults = maximal.stream().filter(candidate ->
                (declarations.get(candidate.owner()).flags().get(candidate) & Opcodes.ACC_ABSTRACT) == 0).toList();
        // A unique inherited default resolves the call. Unrelated abstract contracts remain candidates.
        return defaults.size() == 1 ? Set.of(defaults.getFirst()) : maximal;
    }

    private static Set<String> interfaceClosure(Set<String> roots, Map<String, Declaration> declarations) {
        Set<String> result = new HashSet<>();
        var pending = new java.util.ArrayDeque<>(roots);
        while (!pending.isEmpty()) {
            String next = pending.removeFirst();
            if (!result.add(next)) continue;
            Declaration declaration = declarations.get(next);
            if (declaration != null) pending.addAll(declaration.interfaces());
        }
        return result;
    }

    private static Member resolveField(Member member, Map<String, Declaration> declarations, Set<String> seen) {
        if (!seen.add(member.owner())) return null;
        Declaration owner = declarations.get(member.owner());
        if (owner == null) return null;
        if (owner.members().contains(member)) return member;
        List<String> parents = new ArrayList<>(owner.interfaces());
        if (owner.parent() != null) parents.add(owner.parent());
        for (String parent : parents) {
            Member resolved = resolveField(new Member(false, parent, member.name(), member.descriptor()), declarations, seen);
            if (resolved != null) return resolved;
        }
        return null;
    }

    private static void verify(ClassIndex index, Map<String, Declaration> declarations, Map<Member, Map<Site, Count>> expected) {
        Set<Member> targets = new HashSet<>();
        declarations.values().forEach(value -> targets.addAll(value.members()));
        verify(index, targets, expected);
    }

    static void verify(ClassIndex index, Set<Member> targets, Map<Member, Map<Site, Count>> expected) {
        for (Member member : targets) {
            ReferenceSearchPage page = index.findReferences(member.target(), Integer.MAX_VALUE);
            assertFalse(page.truncated(), member.toString());
            Map<Site, Count> actual = new HashMap<>();
            for (ReferenceResult result : page.results()) {
                assertEquals(ReferenceSiteKind.METHOD, result.kind());
                assertNull(actual.put(new Site(result.ownerInternalName(), result.name(), result.descriptor()),
                        new Count(result.occurrenceCount(), result.kinds())), "Duplicate result site");
            }
            Map<Site, Count> wanted = expected.getOrDefault(member, Map.of());
            if (!wanted.equals(actual)) {
                Set<Site> sites = new HashSet<>(wanted.keySet());
                sites.addAll(actual.keySet());
                List<String> differences = sites.stream().filter(site -> !Objects.equals(wanted.get(site), actual.get(site)))
                        .sorted(java.util.Comparator.comparing(Site::toString)).limit(10)
                        .map(site -> site + " expected=" + wanted.get(site) + " actual=" + actual.get(site)).toList();
                fail(member + " expectedSites=" + wanted.size() + " actualSites=" + actual.size() + " differences=" + differences);
            }
        }
    }

    interface BaseContract { default int ping() { return 1; } }
    interface ChildContract extends BaseContract { @Override default int ping() { return 2; } }
    static class BaseCarrier implements BaseContract { }
    static final class ChildCarrier extends BaseCarrier implements ChildContract { }
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
            new ChildCarrier().ping();
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
