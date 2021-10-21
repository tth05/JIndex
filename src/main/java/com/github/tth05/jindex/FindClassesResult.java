package com.github.tth05.jindex;

import java.util.Arrays;

public class FindClassesResult {

    private final String className;
    private final String[] methodNames;

    public FindClassesResult(String className, String[] methodNames) {
        this.className = className;
        this.methodNames = methodNames;
    }

    public String getClassName() {
        return className;
    }

    public String[] getMethodNames() {
        return methodNames;
    }

    @Override
    public String toString() {
        return "FindClassesResult{" +
               "className='" + className + '\'' +
               ", methodNames=" + Arrays.toString(methodNames) +
               '}';
    }
}
