package com.github.tth05.jindex;

/**
 * Represents any internal exception which may occur during the reading/parsing/deserialization/indexing process.
 */
public class ClassIndexBuildingException extends RuntimeException {
    private static final long serialVersionUID = 1L;

    /**
     * Creates an index-building exception with a descriptive message.
     *
     * @param message the failure description
     */
    public ClassIndexBuildingException(String message) {
        super(message);
    }
}
