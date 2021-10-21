package com.github.tth05.jindex.constantPool;

import com.github.tth05.jindex.utils.StringUtils;

import java.io.ByteArrayOutputStream;
import java.util.Arrays;
import java.util.List;

public class ConstantPool {

    private byte[] data;
    private int writerIndex;

    public ConstantPool(int capacity) {
        this.data = new byte[capacity];
    }

    public ConstantPool(List<IndexString> strings) {
        this.data = strings.stream()
                .collect(ByteArrayOutputStream::new, (baos1, value) -> {
                    byte[] b = value.toByteArray();
                    baos1.write(b.length);
                    baos1.write(b, 0, b.length);
                }, (baos1, baos2) -> {
                    byte[] b = baos2.toByteArray();
                    baos1.write(b, 0, b.length);
                }).toByteArray();
    }

    public int addString(IndexString string) {
        if (this.data.length - this.writerIndex < string.length() + 1)
            this.data = Arrays.copyOf(this.data, this.data.length + string.length() + 1);

        int startIndex = this.writerIndex;
        this.data[this.writerIndex++] = (byte) string.length();
        string.copyInto(this.data, this.writerIndex);
        this.writerIndex += string.length();

        return startIndex;
    }

    public ConstantPoolStringView stringAt(int index) {
        return new ConstantPoolStringView(this, index);
    }

    public static class ConstantPoolStringView implements AsciiCharSequence {

        protected final ConstantPool constantPool;
        protected final int index;

        private ConstantPoolStringView(ConstantPool constantPool, int index) {
            this.constantPool = constantPool;
            this.index = index + 1;
        }

        @Override
        public int length() {
            return this.constantPool.data[this.index - 1] & 0xFF;
        }

        @Override
        public byte byteAt(int index) {
            return this.constantPool.data[this.index + index];
        }

        @Override
        public AsciiCharSequence subSequence(int start, int end) {
            if (start < 0 || end < 0 || (start > end) || end > length())
                throw new IndexOutOfBoundsException();

            return new SubSequence(this.constantPool, this.index + start, this.index + end);
        }

        @Override
        public boolean equals(Object o) {
            if (this == o) return true;
            if (!(o instanceof AsciiCharSequence))
                return false;

            AsciiCharSequence that = (AsciiCharSequence) o;

            int myLength = this.length();
            if (that.length() != myLength)
                return false;

            for (int i = 0; i < myLength; i++) {
                if (this.byteAt(i) != that.byteAt(i))
                    return false;
            }

            return true;
        }

        @Override
        public int hashCode() {
            int result = 1;
            for (int i = this.index; i < this.length(); i++)
                result = 31 * result + this.constantPool.data[i];

            return result;
        }

        public String toJavaString() {
            return StringUtils.toString(this);
        }

        public static final class SubSequence extends ConstantPoolStringView {

            private final int end;

            public SubSequence(ConstantPool constantPool, int start, int end) {
                super(constantPool, start - 1);
                this.end = end;
            }

            @Override
            public int length() {
                return end - this.index;
            }

            @Override
            public byte byteAt(int index) {
                return this.constantPool.data[this.index + index];
            }

            @Override
            public ConstantPoolStringView subSequence(int start, int end) {
                if (start < 0 || end < 0 || (start > end) || end > length())
                    throw new IndexOutOfBoundsException();

                return new SubSequence(this.constantPool, this.index + start, this.index + end);
            }
        }
    }
}
