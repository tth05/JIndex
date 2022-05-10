package com.github.tth05.jindex;

/**
 * Represents any internal exception which may occur during the reading/parsing/deserialization/indexing process.
 */
public class ClassIndexBuildingException extends RuntimeException{

    public ClassIndexBuildingException(String message) {
        super(message);
    }
}
