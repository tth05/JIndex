package com.github.tth05.jindex;

public enum InnerClassType {
    /**
     * Inner class within a class
     */
    MEMBER,
    /**
     * Anonymous inner class. Can be in a method or a class.
     */
    ANONYMOUS,
    /**
     * Inner class within a method
     */
    LOCAL
}
