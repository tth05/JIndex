package com.github.tth05.jindex;

import org.objectweb.asm.ClassReader;

import java.net.URI;
import java.nio.file.FileSystems;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashSet;
import java.util.HexFormat;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.jar.JarFile;
import java.util.zip.ZipFile;

import static com.github.tth05.jindex.ReferenceExtractionOracleTest.*;

/** Independent ASM member extraction over every selected class in a fixed runtime capture. */
public final class RuntimeReferenceOracle {
    private RuntimeReferenceOracle() { }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length != 2) throw new IllegalArgumentException("Expected <manifest> <result-json>");
        Path manifest = Path.of(arguments[0]);
        List<String> lines = Files.readAllLines(manifest);
        if (lines.isEmpty() || !lines.getFirst().equals("totaldebug-runtime-sources-v1")) {
            throw new IllegalArgumentException("Invalid runtime source manifest");
        }
        List<Path> archives = lines.stream().skip(1).map(URI::create).map(Path::of).toList();
        Map<String, byte[]> selected = new LinkedHashMap<>();
        Path modules = FileSystems.getFileSystem(URI.create("jrt:/")).getPath("/modules");
        try (var files = Files.walk(modules)) {
            for (Path path : files.filter(Files::isRegularFile).sorted().toList()) {
                if (indexable(path.getFileName().toString())) add(selected, Files.readAllBytes(path));
            }
        }
        List<byte[]> jdk = List.copyOf(selected.values());
        for (Path archive : archives) {
            // JarFile supplies the Java runtime's multi-release view independently of the native ZIP reader.
            try (JarFile jar = new JarFile(archive.toFile(), false, ZipFile.OPEN_READ, Runtime.version())) {
                for (var entry : jar.versionedStream().filter(entry -> !entry.isDirectory()
                        && indexable(entry.getName())).sorted(Comparator.comparing(java.util.jar.JarEntry::getName)).toList()) {
                    try (var input = jar.getInputStream(entry)) { add(selected, input.readAllBytes()); }
                }
            }
        }
        List<byte[]> classes = List.copyOf(selected.values());
        System.out.println("ASM corpus selected classes=" + classes.size() + " archives=" + archives.size());
        Map<String, Declaration> declarations = declarations(classes);
        Set<Member> targets = new HashSet<>();
        for (String owner : List.of("net/minecraft/world/level/block/Block",
                "net/minecraft/world/level/block/Blocks", "java/lang/String", "java/util/List")) {
            Declaration declaration = declarations.get(owner);
            if (declaration == null) throw new IllegalStateException("Missing oracle target class " + owner);
            targets.addAll(declaration.members());
        }
        Map<Member, Map<Site, Count>> expected = extract(classes, declarations, targets);
        long sites = expected.values().stream().mapToLong(Map::size).sum();
        if (sites < 10_000) throw new IllegalStateException("Runtime oracle lost expected corpus coverage");
        System.out.println("ASM corpus target members=" + targets.size() + " sites=" + sites);
        Path snapshot = Files.createTempFile("jindex-asm-corpus-", ".index");
        try {
            try (ClassIndex index = ClassIndex.fromSources(archives.stream().map(Path::toString).toList(), jdk)) {
                if (index.getStatistics().classCount() != classes.size()) {
                    throw new AssertionError("ASM/native source selection differs: " + classes.size()
                            + " versus " + index.getStatistics().classCount());
                }
                verify(index, targets, expected);
                index.saveToFile(snapshot.toString());
            }
            try (ClassIndex index = ClassIndex.fromFile(snapshot.toString())) { verify(index, targets, expected); }
        } finally {
            Files.deleteIfExists(snapshot);
        }
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        classes.forEach(digest::update);
        List<String> results = new ArrayList<>();
        for (Member target : targets.stream().sorted(Comparator.comparing(Member::toString)).toList()) {
            Map<Site, Count> references = expected.getOrDefault(target, Map.of());
            long occurrences = references.values().stream().mapToLong(Count::occurrences).sum();
            results.add("    {\"target\": " + quote(target.toString()) + ", \"sites\": " + references.size()
                    + ", \"occurrences\": " + occurrences + "}");
        }
        String report = "{\n  \"schema\": \"jindex-asm-member-corpus-v1\",\n  \"javaVersion\": "
                + quote(System.getProperty("java.version")) + ",\n  \"manifestSha256\": "
                + quote(HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(Files.readAllBytes(manifest))))
                + ",\n  \"selectedClassesSha256\": " + quote(HexFormat.of().formatHex(digest.digest()))
                + ",\n  \"selectedClassCount\": " + classes.size() + ",\n  \"archiveCount\": " + archives.size()
                + ",\n  \"beforeAndAfterSnapshotMatch\": true,\n  \"targets\": [\n" + String.join(",\n", results) + "\n  ]\n}\n";
        Files.writeString(Path.of(arguments[1]), report);
        System.out.println("ASM corpus passed before and after snapshot reload");
    }

    private static boolean indexable(String name) {
        return name.endsWith(".class") && !name.equals("module-info.class") && !name.endsWith("/module-info.class");
    }

    private static void add(Map<String, byte[]> selected, byte[] bytes) {
        selected.putIfAbsent(new ClassReader(bytes).getClassName(), bytes);
    }

    private static String quote(String value) {
        return "\"" + value.replace("\\", "\\\\").replace("\"", "\\\"") + "\"";
    }
}
