# JIndex

Java class file indexing library, implemented in Rust with JNI bindings. The implementation was made in Rust to greatly
reduce memory consumption and increase indexing speed.

## Features

The current implementation supports indexing of the following data:

- Packages
    - Package name
    - Parent package
    - Contained classes
    - Sub-packages
- Classes
    - Package
    - Name and Source Name (for inner classes)
    - Super class
    - Implemented interfaces
    - Modifiers
    - Inner class type (member, anonymous, local)
    - Generic signature
    - Enclosing class (and method)
    - Inner classes with the member type
    - Methods
        - Name
        - Modifiers
        - Generic signature and descriptor
        - Exceptions
        - Parameter types
        - Return type
    - Fields
        - Name
        - Generic signature and descriptor

The following global operations are supported: 
- Find a class
- Find classes by name matching a query
- Find a package
- Find packages by prefix
- Find implementations of a class
- Find implementations of a method
- Find base methods of a method

After the indexing operation is complete, no further modifications to the class index are possible. The whole library
only works with ASCII strings. Supplying a non ASCII string will result in an error, or it will be ignored.

NOTE: The Java bindings are incomplete and don't expose all data as usable objects (e.g. generic signatures are only
available as strings).

## Performance

JIndex was made with [TotalDebugCompanion](https://github.com/Minecraft-TA/TotalDebugCompanion) in mind with the goal
to be fast and also be able to fit the indexed data nicely into memory.

It has been tested with a set of around 330 jars containing 175k classes, 1.2 million methods and 500k fields. These
take roughly 2.2 seconds to index on modern CPU, a quarter of this time is spent on file reading. The resulting index amounts
to 256MB of memory. When serialized, the index becomes a 13MB file (54MB uncompressed) with a deserialization time of
500ms.

## Usage

If anyone wants to use this (not recommended), the java bindings are available on packagecloud. Check the tags for the
newest version.

```grooy
repositories {
    maven { url "https://packagecloud.io/tth05/repo/maven2" }
}

dependencies {
    implementation("com.github.tth05:jindex:VERSION")
}
```

## Development

- Clone the repo
- Run `cargo build` or `cargo test` for the rust side
- Run the `copyNativeLibrary` task and then any test for the java side
