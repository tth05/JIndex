package com.github.tth05.jindex.constantPool;

import com.github.tth05.jindex.utils.StringUtils;

import java.util.Arrays;

public class IndexString implements AsciiCharSequence {

    protected final byte[] data;
    protected int hashCode = -1;

    private IndexString(byte[] data) {
        this.data = data;
    }

    public IndexString(String template) {
        char[] chars = template.toCharArray();

        this.data = new byte[chars.length];
        for (int i = 0; i < chars.length; i++) {
            char b = chars[i];
            if (b > 127)
                throw new IllegalArgumentException("Unicode not supported");
            this.data[i] = (byte) b;
        }
    }

    @Override
    public int length() {
        return this.data.length;
    }

    @Override
    public byte byteAt(int index) {
        return this.data[index];
    }

    @Override
    public IndexString subSequence(int start, int end) {
        if (start < 0 || end < 0 || (start > end) || end > length())
            throw new IndexOutOfBoundsException();

        return new SubSequence(this.data, start, end);
    }

    @Override
    public IndexString subSequenceAfterLast(byte separator) {
        return (IndexString) AsciiCharSequence.super.subSequenceAfterLast(separator);
    }

    @Override
    public IndexString subSequenceBeforeLast(byte separator) {
        return (IndexString) AsciiCharSequence.super.subSequenceBeforeLast(separator);
    }

    public void copyInto(byte[] dest, int offset) {
        System.arraycopy(this.data, 0, dest, offset, this.data.length);
    }

    public byte[] toByteArray() {
        return Arrays.copyOf(this.data, this.data.length);
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof AsciiCharSequence)) return false;

        AsciiCharSequence that = (AsciiCharSequence) o;
        if (that.length() != this.length())
            return false;

        for (int i = 0; i < this.length(); i++) {
            if (this.byteAt(i) != that.byteAt(i))
                return false;
        }

        return true;
    }

    @Override
    public int hashCode() {
        return this.hashCode == -1 ? (this.hashCode = Arrays.hashCode(this.data)) : this.hashCode;
    }

    @Override
    public String toString() {
        return "IndexString{'" + StringUtils.toString(this) + "'}";
    }

    public static final class SubSequence extends IndexString {

        private final int start;
        private final int end;

        public SubSequence(byte[] data, int start, int end) {
            super(data);
            this.start = start;
            this.end = end;
        }

        @Override
        public int length() {
            return end - start;
        }

        @Override
        public byte byteAt(int index) {
            return this.data[start + index];
        }

        @Override
        public IndexString subSequence(int start, int end) {
            if (start < 0 || end < 0 || (start > end) || end > length())
                throw new IndexOutOfBoundsException();

            return new SubSequence(this.data, this.start + start, this.start + end);
        }

        @Override
        public void copyInto(byte[] dest, int offset) {
            System.arraycopy(this.data, this.start, dest, offset, length());
        }

        @Override
        public byte[] toByteArray() {
            return Arrays.copyOfRange(this.data, this.start, this.end);
        }

        @Override
        public int hashCode() {
            if (this.hashCode == -1) {
                int result = 1;
                for (int i = this.start; i < this.end; i++)
                    result = 31 * result + this.data[i];
                this.hashCode = result;
            }

            return this.hashCode;
        }
    }
}
