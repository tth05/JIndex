package com.github.tth05.jindex;

/** Options that determine which runtime view of the inputs is indexed. */
public record IndexBuildOptions(int targetJavaRelease) {

    public IndexBuildOptions {
        if (targetJavaRelease < 1) {
            throw new IllegalArgumentException("targetJavaRelease must be positive");
        }
    }

    public static IndexBuildOptions currentRuntime() {
        return new IndexBuildOptions(Runtime.version().feature());
    }
}
