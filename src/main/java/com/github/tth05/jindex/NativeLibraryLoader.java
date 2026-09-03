package com.github.tth05.jindex;

import java.io.IOException;
import java.io.InputStream;
import java.io.ByteArrayInputStream;
import java.nio.channels.FileChannel;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.nio.file.StandardOpenOption;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.HexFormat;

/** Content-addressed extraction with separate native field-ID caches for isolated class loaders. */
final class NativeLibraryLoader {
    private NativeLibraryLoader() {}

    static Path cacheDirectory() {
        String configured = System.getProperty("jindex.native.cacheDir");
        return configured == null
                ? Path.of(System.getProperty("user.home"), ".cache", "jindex", "natives")
                : Path.of(configured);
    }

    static void load(InputStream resource, Path directory) throws IOException {
        byte[] bytes = resource.readAllBytes();
        String key = "com.github.tth05.jindex.nativeLoaderCount." + HexFormat.of().formatHex(digest(bytes));
        // Properties are JVM-wide even when this helper is loaded by isolated class loaders.
        // Keep the value a String so ordinary System.getProperties() consumers remain valid.
        var properties = System.getProperties();
        int slot;
        synchronized (properties) {
            slot = Integer.parseInt(properties.getProperty(key, "0"));
            properties.setProperty(key, Integer.toString(Math.incrementExact(slot)));
        }
        System.load(extract(new ByteArrayInputStream(bytes), directory, slot).toString());
    }

    // Synchronization handles callers in this class loader; the file lock coordinates JVMs.
    static synchronized Path extract(InputStream resource, Path directory) throws IOException {
        return extract(resource, directory, 0);
    }

    private static synchronized Path extract(InputStream resource, Path directory, int slot) throws IOException {
        byte[] bytes = resource.readAllBytes();
        byte[] hash = digest(bytes);
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
