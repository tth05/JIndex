package com.github.tth05.jindex.search;

import com.github.tth05.jindex.constantPool.AsciiCharSequence;

import java.util.*;

public class PrefixTree<T> {

    private final Node<T> rootNode;

    public PrefixTree(byte maxDepth) {
        this.rootNode = new Node<>(maxDepth);
    }

    public void put(AsciiCharSequence key, T value) {
        rootNode.put(key, value);
    }

    public List<T> findAllStartingWith(AsciiCharSequence query) {
        ArrayList<T> results = new ArrayList<>();
        this.rootNode.collectAllStartingWith(query, results);
        return results;
    }

    private static byte byteAsMaskByte(byte b) {
        if (b >= 97 && b <= 122) {
            return (byte) (b - 97);
        }
        if (b >= 65 && b <= 90) {
            return (byte) (b - 65 + 26);
        }
        if (b >= 48 && b <= 57) {
            return (byte) (b - 48 + 26 * 2);
        }
        if (b == 36) {
            return (byte) (26 * 2 + 10);
        }
        if (b == 95) {
            return (byte) (26 * 2 + 10 + 1);
        }

        throw new IllegalArgumentException("The prefix tree only supports [a-zA-Z0-9$_]");
    }

    private interface PrefixTreeNode<T> {

        void put(AsciiCharSequence key, T value);

        void collectAllStartingWith(AsciiCharSequence sequence, ArrayList<T> results);
    }


    private static final class TailNode<T> implements PrefixTreeNode<T> {

        private final List<AsciiCharSequence> keyList = new ArrayList<>();
        private final List<T> valueList = new ArrayList<>();

        @Override
        public void put(AsciiCharSequence key, T value) {
            keyList.add(key);
            valueList.add(value);
        }

        @Override
        public void collectAllStartingWith(AsciiCharSequence sequence, ArrayList<T> results) {
            for (int i = 0; i < this.keyList.size(); i++) {
                if (this.keyList.get(i).startsWith(sequence))
                    results.add(valueList.get(i));
            }
        }
    }

    private static class Node<T> implements PrefixTreeNode<T> {

        private long mask;
        private final byte depth;
        private PrefixTreeNode<T>[] children;
        private T[] values;

        public Node(byte depth) {
            this.depth = depth;
        }

        public void put(AsciiCharSequence key, T value) {
            if (key.length() == 0) {
                addValue(value);
                return;
            }

            byte currentByte = key.byteAt(0);
            int index = getIndexForByte(currentByte);

            PrefixTreeNode<T> node;
            if (!hasChildForByte(currentByte)) {
                node = createNewNode();
                insertNode(index, byteAsMaskByte(currentByte), node);
            } else {
                node = this.children[index];
            }

            node.put(key.subSequence(1, key.length()), value);
        }

        private void addValue(T value) {
            if (this.values == null) {
                this.values = (T[]) new Object[]{value};
                return;
            }

            this.values = Arrays.copyOf(this.values, this.values.length + 1);
            this.values[this.values.length - 1] = value;
        }

        public void collectAllStartingWith(AsciiCharSequence sequence, ArrayList<T> results) {
            if (sequence.length() == 0) {
                if (this.values != null) {
                    results.ensureCapacity(results.size() + this.values.length);
                    //#addAll would create an Object and do a full array copy...
                    //noinspection ManualArrayToCollectionCopy
                    for (T value : this.values) {
                        //noinspection UseBulkOperation
                        results.add(value);
                    }
                }

                if (this.children != null) {
                    for (PrefixTreeNode<T> child : this.children)
                        child.collectAllStartingWith(sequence, results);
                }
            } else {
                if (!hasChildForByte(sequence.byteAt(0)))
                    return;

                this.children[getIndexForByte(sequence.byteAt(0))].collectAllStartingWith(sequence.subSequence(1, sequence.length()), results);
            }
        }

        private void insertNode(int index, byte maskByte, PrefixTreeNode<T> newNode) {
            if (this.children == null) {
                this.children = new PrefixTreeNode[]{newNode};
                this.mask |= 1L << maskByte;
                return;
            }

            PrefixTreeNode<T>[] oldNodes = this.children;
            this.children = new PrefixTreeNode[oldNodes.length + 1];

            System.arraycopy(oldNodes, 0, this.children, 0, index);
            this.children[index] = newNode;
            System.arraycopy(oldNodes, index, this.children, index + 1, oldNodes.length - index);

            this.mask |= 1L << maskByte;
        }

        private PrefixTreeNode<T> createNewNode() {
            if (this.depth == 1)
                return new TailNode<>();
            else
                return new Node<>((byte) (this.depth - 1));
        }

        private boolean hasChildForByte(byte b) {
            return ((mask >> byteAsMaskByte(b)) & 1) != 0;
        }

        private int getIndexForByte(byte b) {
            byte maskByte = byteAsMaskByte(b);
            if (maskByte == 0)
                return 0;
            return Long.bitCount((0xFFFFFFFFFFFFFFFFL >>> (64 - maskByte)) & mask);
        }
    }
}
