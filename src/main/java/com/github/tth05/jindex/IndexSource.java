package com.github.tth05.jindex;

import java.util.Objects;

/**
 * One ordered input to a class index build. The source ID is opaque to JIndex and may be shared by
 * many inputs, such as every class in one JDK module.
 */
public sealed interface IndexSource permits IndexSource.Archive, IndexSource.ClassFile {

    int sourceId();

    static Archive archive(int sourceId, String path) {
        return new Archive(sourceId, path);
    }

    static ClassFile classFile(int sourceId, byte[] bytes) {
        return new ClassFile(sourceId, bytes);
    }

    record Archive(int sourceId, String path) implements IndexSource {
        public Archive {
            requireValidSourceId(sourceId);
            Objects.requireNonNull(path, "path");
        }
    }

    /**
     * The byte array is read synchronously during the build and is not copied. Callers must not
     * modify it until {@link ClassIndex#fromSources(java.util.List)} returns.
     */
    record ClassFile(int sourceId, byte[] bytes) implements IndexSource {
        public ClassFile {
            requireValidSourceId(sourceId);
            Objects.requireNonNull(bytes, "bytes");
        }
    }

    private static void requireValidSourceId(int sourceId) {
        if (sourceId < 0) {
            throw new IllegalArgumentException("sourceId must be non-negative");
        }
    }
}
