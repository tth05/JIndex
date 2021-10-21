package com.github.tth05.jindex.constantPool;

public interface AsciiCharSequence {

    int length();

    byte byteAt(int index);

    AsciiCharSequence subSequence(int start, int end);

    default AsciiCharSequence subSequenceAfterLast(byte separator) {
        int i = lastIndexOf(separator);
        return i == -1 || i == length() - 1 ? this : subSequence(i + 1, length());
    }

    default AsciiCharSequence subSequenceBeforeLast(byte separator) {
        int i = lastIndexOf(separator);
        return i == -1 || i == length() - 1 ? this : subSequence(0, i);
    }

    default int indexOf(byte c) {
        for (int i = 0, length = length(); i < length - 1; i++) {
            if (byteAt(i) == c)
                return i;
        }
        return -1;
    }

    default int lastIndexOf(byte c) {
        for (int i = length() - 1; i >= 0; i--) {
            if (byteAt(i) == c)
                return i;
        }
        return -1;
    }

    default boolean startsWith(AsciiCharSequence sequence) {
        if (sequence.length() > length())
            return false;

        for (int i = 0; i < sequence.length(); i++) {
            if (byteAt(i) != sequence.byteAt(i))
                return false;
        }

        return true;
    }
}
