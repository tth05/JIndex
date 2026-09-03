package com.github.tth05.jindex;

import java.io.IOException;
import java.io.InputStream;
import java.nio.channels.FileChannel;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.nio.file.StandardOpenOption;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.HexFormat;

/**
 * Extracts the bundled native library into a content-addressed cache and loads it.
 * <p>
 * Windows keeps a loaded DLL locked, so a temporary copy per JVM could never be deleted. The cache
 * path is derived from the library's SHA-256, so identical builds share one file across JVMs and
 * a different build never overwrites a loaded one. The JVM refuses to load one DLL path into more
 * than one class loader, so each additional loader in a JVM receives its own numbered copy.
 */
final class NativeLibraryLoader {
    private static final String LOADER_SLOT_PROPERTY = "com.github.tth05.jindex.nativeLoaderCount.";

    private NativeLibraryLoader() {}

    static Path cacheDirectory() {
        String configured = System.getProperty("jindex.native.cacheDir");
        return configured == null
                ? Path.of(System.getProperty("user.home"), ".cache", "jindex", "natives")
                : Path.of(configured);
    }

    static void load(InputStream resource, Path directory) throws IOException {
        byte[] bytes = resource.readAllBytes();
        byte[] hash = digest(bytes);
        System.load(extract(bytes, hash, directory, claimLoaderSlot(hash)).toString());
    }

    /**
     * System properties are JVM-wide even when this class is loaded by isolated class loaders, so
     * they act as the per-library loader counter. The value stays a String so ordinary
     * {@code System.getProperties()} consumers remain valid.
     */
    private static int claimLoaderSlot(byte[] hash) {
        String key = LOADER_SLOT_PROPERTY + HexFormat.of().formatHex(hash);
        var properties = System.getProperties();
        synchronized (properties) {
            int slot = Integer.parseInt(properties.getProperty(key, "0"));
            properties.setProperty(key, Integer.toString(Math.incrementExact(slot)));
            return slot;
        }
    }

    static Path extract(byte[] bytes, Path directory) throws IOException {
        return extract(bytes, digest(bytes), directory, 0);
    }

    // Synchronization handles callers in this class loader; the file lock coordinates JVMs.
    private static synchronized Path extract(byte[] bytes, byte[] hash, Path directory, int slot)
            throws IOException {
        String name = "jindex_rs-" + HexFormat.of().formatHex(hash) + (slot == 0 ? "" : "-loader" + slot);
        Files.createDirectories(directory);
        Path target = directory.resolve(name + ".dll").toAbsolutePath();
        try (FileChannel channel = FileChannel.open(directory.resolve(name + ".lock"),
                StandardOpenOption.CREATE, StandardOpenOption.WRITE);
             var lock = channel.lock()) {
            if (!lock.isValid()) {
                throw new IOException("Unable to lock JIndex native cache: " + directory);
            }
            if (Files.exists(target, LinkOption.NOFOLLOW_LINKS)) {
                if (!Files.isRegularFile(target, LinkOption.NOFOLLOW_LINKS)
                        || !MessageDigest.isEqual(hash, digest(Files.readAllBytes(target)))) {
                    throw new IOException("JIndex native cache checksum mismatch: " + target);
                }
                return target.toRealPath();
            }
            Path temporary = Files.createTempFile(directory, name + "-", ".tmp");
            try {
                Files.write(temporary, bytes);
                Files.move(temporary, target, StandardCopyOption.ATOMIC_MOVE);
            } finally {
                Files.deleteIfExists(temporary);
            }
            return target.toRealPath();
        }
    }

    private static byte[] digest(byte[] bytes) {
        try {
            return MessageDigest.getInstance("SHA-256").digest(bytes);
        } catch (NoSuchAlgorithmException exception) {
            throw new AssertionError("Java 21 must provide SHA-256", exception);
        }
    }
}
