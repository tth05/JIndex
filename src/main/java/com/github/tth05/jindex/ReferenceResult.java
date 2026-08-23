package com.github.tth05.jindex;

import java.util.Objects;

/**
 * One declaration site containing one or more references to a target symbol.
 *
 * @param siteId index-local identity of the declaration site
 * @param kind kind of declaration containing the references
 * @param ownerInternalName internal name of the class containing the declaration
 * @param name member or record-component name, or an empty string for a class site
 * @param descriptor JVM descriptor, or an empty string for a class site
 * @param sourceId opaque ID of the source that supplied the containing class
 * @param occurrenceCount number of references from this declaration site to the target
 */
public record ReferenceResult(
        long siteId,
        ReferenceSiteKind kind,
        String ownerInternalName,
        String name,
        String descriptor,
        int sourceId,
        long occurrenceCount
) {
    ReferenceResult(
            long siteId,
            int kind,
            String ownerInternalName,
            String name,
            String descriptor,
            int sourceId,
            long occurrenceCount
    ) {
        this(
                siteId,
                ReferenceSiteKind.values()[kind],
                Objects.requireNonNull(ownerInternalName, "ownerInternalName"),
                Objects.requireNonNull(name, "name"),
                Objects.requireNonNull(descriptor, "descriptor"),
                sourceId,
                occurrenceCount
        );
    }
}
