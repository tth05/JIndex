package com.github.tth05.jindex;

/**
 * This class holds information about the build time of a class index.
 */
public class BuildTimeInfo {

    private final long deserializationTime;
    private final long classReadingTime;
    private final long indexingTime;

    private BuildTimeInfo(long deserializationTime, long classReadingTime, long indexingTime) {
        this.deserializationTime = deserializationTime;
        this.classReadingTime = classReadingTime;
        this.indexingTime = indexingTime;
    }

    /**
     * @return The deserialization time in milliseconds, if the index was deserialized from a file; {@code 0} otherwise
     */
    public long getDeserializationTime() {
        return deserializationTime;
    }

    /**
     * @return The class parsing time in milliseconds
     */
    public long getClassReadingTime() {
        return classReadingTime;
    }

    /**
     * @return The indexing time in milliseconds of the parsed classes
     */
    public long getIndexingTime() {
        return indexingTime;
    }

    /**
     * @return The total time in milliseconds
     */
    public long getTotalTime() {
        return deserializationTime + classReadingTime + indexingTime;
    }

    /**
     * @return An opinionated string representation of the build time information
     */
    public String toFormattedString() {
        return String.format("Deserialization time: %dms\nClass reading time: %dms\nIndexing time: %dms\nTotal time: %dms",
                deserializationTime, classReadingTime, indexingTime, getTotalTime());
    }

    @Override
    public String toString() {
        return "BuildTimeInfo{" +
               "deserializationTime=" + deserializationTime +
               ", classReadingTime=" + classReadingTime +
               ", indexingTime=" + indexingTime +
               '}';
    }
}
