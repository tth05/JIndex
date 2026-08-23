package com.github.tth05.jindex;

import java.util.Objects;

/** A declaration whose incoming references should be returned. */
public sealed interface ReferenceTarget permits ReferenceTarget.ClassTarget, ReferenceTarget.FieldTarget,
        ReferenceTarget.MethodTarget {

    String ownerInternalName();

    SymbolKind kind();

    String name();

    String descriptor();

    static ClassTarget classTarget(String internalName) {
        return new ClassTarget(internalName);
    }

    static FieldTarget fieldTarget(String ownerInternalName, String name, String descriptor) {
        return new FieldTarget(ownerInternalName, name, descriptor);
    }

    static MethodTarget methodTarget(String ownerInternalName, String name, String descriptor) {
        return new MethodTarget(ownerInternalName, name, descriptor);
    }

    record ClassTarget(String ownerInternalName) implements ReferenceTarget {
        public ClassTarget {
            Objects.requireNonNull(ownerInternalName, "ownerInternalName");
        }

        @Override
        public SymbolKind kind() {
            return SymbolKind.CLASS;
        }

        @Override
        public String name() {
            return "";
        }

        @Override
        public String descriptor() {
            return "";
        }
    }

    record FieldTarget(String ownerInternalName, String name, String descriptor) implements ReferenceTarget {
        public FieldTarget {
            requireMember(ownerInternalName, name, descriptor);
        }

        @Override
        public SymbolKind kind() {
            return SymbolKind.FIELD;
        }
    }

    record MethodTarget(String ownerInternalName, String name, String descriptor) implements ReferenceTarget {
        public MethodTarget {
            requireMember(ownerInternalName, name, descriptor);
        }

        @Override
        public SymbolKind kind() {
            return SymbolKind.METHOD;
        }
    }

    private static void requireMember(String ownerInternalName, String name, String descriptor) {
        Objects.requireNonNull(ownerInternalName, "ownerInternalName");
        Objects.requireNonNull(name, "name");
        Objects.requireNonNull(descriptor, "descriptor");
    }
}
