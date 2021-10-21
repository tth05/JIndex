package com.github.tth05.jindex.utils;

import com.github.tth05.jindex.constantPool.AsciiCharSequence;

public class StringUtils {

    public static String toString(AsciiCharSequence charSequence) {
        char[] array = new char[charSequence.length()];
        for (int i = 0; i < charSequence.length(); i++)
            array[i] = (char) charSequence.byteAt(i);

        return new String(array);
    }

    public static String substringAfter(String s, String separator) {
        int index = s.lastIndexOf(separator);
        return index == -1 ? s : s.substring(index + 1);
    }
}
