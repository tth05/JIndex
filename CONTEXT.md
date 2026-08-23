# JIndex

JIndex turns a fixed set of JVM class sources into an immutable, searchable snapshot of their declarations and semantic uses.

## Language

**Class source**:
One caller-supplied archive or class group from which JIndex selects class definitions. A class source has an index-local identity but no application-specific meaning.
_Avoid_: Mod, library, JAR owner

**Declaration**:
A class, field, or method defined by an indexed class file.
_Avoid_: Definition, element

**Symbol**:
The stable identity of one declaration within an index snapshot. Symbols are distinguished by kind, declaring class, name, and descriptor where applicable.
_Avoid_: Item, entry, node

**Reference**:
A semantic use of a target symbol by an indexed class file. A reference records its target and reference site, not the complete bytecode instruction that produced it.
_Avoid_: Usage, dependency, call

**Reference site**:
The class, field, or method declaration containing a reference.
_Avoid_: Caller, source symbol

**String literal**:
A Java string value used semantically by a class file, such as an `ldc` value, annotation value, or field constant. Arbitrary UTF-8 constants in the class-file constant pool are not string literals.
_Avoid_: Text, UTF-8 constant

**Literal occurrence**:
A use of a string literal at a reference site.
_Avoid_: String reference

**Index snapshot**:
The immutable result of indexing one ordered set of class sources. Every ID is valid only inside the snapshot that assigned it.
_Avoid_: Database, cache file

**Provenance**:
Caller-owned metadata that gives a class source application-specific meaning, such as a mod, platform component, library, or JDK module. JIndex preserves the source identity but does not interpret provenance.
_Avoid_: Source name, mod metadata
