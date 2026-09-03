package com.github.tth05.jindex;

import org.junit.jupiter.api.Test;

import java.lang.ref.Reference;
import java.lang.ref.WeakReference;
import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.List;
import java.util.Objects;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;
import java.util.concurrent.locks.LockSupport;

import static org.junit.jupiter.api.Assertions.*;

final class NativeLifetimeTest {
    @Test
    void closeWaitsForChildOperationsAndRejectsLaterCalls() throws Exception {
        ClassIndex index = newIndex();
        IndexedClass child = Objects.requireNonNull(index.findClass(Fixture.class.getName()));
        var entered = new CountDownLatch(1);
        var release = new CountDownLatch(1);
        var closing = new CountDownLatch(1);
        try (var executor = Executors.newFixedThreadPool(2)) {
            try {
                var operation = executor.submit(() -> child.executeWhileOwnerOpen(() -> {
                    entered.countDown();
                    await(release);
                    return child.getName();
                }));
                assertTrue(entered.await(5, TimeUnit.SECONDS));
                var close = executor.submit(() -> { closing.countDown(); index.close(); });
                assertTrue(closing.await(5, TimeUnit.SECONDS));
                assertThrows(TimeoutException.class, () -> close.get(100, TimeUnit.MILLISECONDS));
                release.countDown();
                assertNotNull(operation.get(5, TimeUnit.SECONDS));
                close.get(5, TimeUnit.SECONDS);
                assertThrows(IllegalStateException.class, child::getName);
            } finally {
                release.countDown();
                index.close();
            }
        }
    }

    @Test
    void repeatedConcurrentCloseAndQueriesDoNotDoubleFree() throws Exception {
        try (var executor = Executors.newFixedThreadPool(6)) {
            for (int iteration = 0; iteration < 100; iteration++) {
                ClassIndex index = newIndex();
                IndexedClass child = Objects.requireNonNull(index.findClass(Fixture.class.getName()));
                var start = new CountDownLatch(1);
                var tasks = new ArrayList<java.util.concurrent.Future<?>>();
                for (int worker = 0; worker < 3; worker++) {
                    tasks.add(executor.submit(() -> {
                        await(start);
                        for (int query = 0; query < 100; query++) {
                            try { child.getMethods(); child.getName(); }
                            catch (IllegalStateException closed) { break; }
                        }
                    }));
                    tasks.add(executor.submit(() -> { await(start); index.close(); index.close(); }));
                }
                start.countDown();
                try {
                    for (var task : tasks) task.get(5, TimeUnit.SECONDS);
                    assertTrue(index.isDestroyed());
                    assertThrows(IllegalStateException.class, child::getName);
                } finally {
                    index.close();
                }
            }
        }
    }

    @Test
    void eachChildKindKeepsItsOwnerAlive() throws Exception {
        for (int kind = 0; kind < 4; kind++) {
            Retained retained = retainedChild(kind);
            try {
                System.gc();
                assertFalse(retained.owner().refersTo(null));
                assertNotNull(retained.child().executeWhileOwnerOpen(() -> "alive"));
                Reference.reachabilityFence(retained.child());
            } finally {
                Objects.requireNonNull(retained.owner().get()).close();
            }
        }
    }

    @Test
    void cleanerDoesNotRetainItsOwnerAndRemovesTheRegistryEntry() throws Exception {
        Abandoned abandoned = abandonIndex();
        long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(10);
        while ((!abandoned.owner().refersTo(null) || registered(abandoned.pointer())) && System.nanoTime() < deadline) {
            System.gc();
            LockSupport.parkNanos(TimeUnit.MILLISECONDS.toNanos(10));
        }
        assertTrue(abandoned.owner().refersTo(null), "Cleaner action must not retain its owner");
        assertFalse(registered(abandoned.pointer()), "Cleaner must claim and remove the native allocation");
    }

    @Test
    void nullNativePointersAreHandledAtTheBoundary() throws Exception {
        var destroy = ClassIndex.class.getDeclaredMethod("destroyPointer", long.class);
        destroy.setAccessible(true);
        assertDoesNotThrow(() -> destroy.invoke(null, 0L));
        try (ClassIndex index = newIndex()) {
            IndexedClass child = Objects.requireNonNull(index.findClass(Fixture.class.getName()));
            Field pointer = ClassIndexChildObject.class.getDeclaredField("pointer");
            pointer.setAccessible(true);
            pointer.setLong(child, 0L);
            assertThrows(RuntimeException.class, child::getName);
        }
    }

    private static boolean registered(long pointer) throws ReflectiveOperationException {
        Field registry = ClassIndex.class.getDeclaredField("OWNERS");
        registry.setAccessible(true);
        return ((java.util.Map<?, ?>) registry.get(null)).containsKey(pointer);
    }

    private static Abandoned abandonIndex() throws Exception {
        ClassIndex index = newIndex();
        return new Abandoned(new WeakReference<>(index), index.classIndexPointer());
    }

    private static Retained retainedChild(int kind) throws Exception {
        ClassIndex index = newIndex();
        IndexedClass type = Objects.requireNonNull(index.findClass(Fixture.class.getName()));
        ClassIndexChildObject child = switch (kind) {
            case 0 -> type;
            case 1 -> type.getPackage();
            case 2 -> type.getFields()[0];
            case 3 -> type.getMethods()[0];
            default -> throw new AssertionError(kind);
        };
        return new Retained(new WeakReference<>(index), child);
    }

    private static ClassIndex newIndex() throws Exception {
        try (var input = Objects.requireNonNull(Fixture.class.getResourceAsStream('/' + Fixture.class.getName().replace('.', '/') + ".class"))) {
            return ClassIndex.fromBytes(List.of(input.readAllBytes()));
        }
    }

    private static void await(CountDownLatch latch) {
        try {
            if (!latch.await(5, TimeUnit.SECONDS)) throw new AssertionError("Latch timed out");
        } catch (InterruptedException interrupted) {
            Thread.currentThread().interrupt();
            throw new AssertionError(interrupted);
        }
    }

    private record Retained(WeakReference<ClassIndex> owner, ClassIndexChildObject child) {}
    private record Abandoned(WeakReference<ClassIndex> owner, long pointer) {}
    private static class Fixture { int value; void run() {} }
}
