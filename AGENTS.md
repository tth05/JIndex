# Working in JIndex

Use Java 21, the checked-in Gradle wrapper and the Rust toolchain in jindex-rs/rust-toolchain.toml. The wrapper builds and stages the native library.

Changes across Java and JNI must preserve native ownership, index-local IDs and source precedence. Verify the affected Java and Rust behavior; native changes also require rustfmt and Clippy.

For coordinated TotalDebug development, publish JIndex to Maven Local before verifying Companion with its local dependency option. Keep application-specific provenance and storage paths in the consumer.

Preserve unrelated working-tree changes and coordinate overlapping edits.
