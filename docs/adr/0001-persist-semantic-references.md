---
status: accepted
---

# Persist semantic references in JIndex

JIndex will parse bytecode once while building an index snapshot and persist compact declaration, reference, and string-literal indexes. It will store resolved symbol IDs and reference sites rather than complete instructions. This replaces repeated whole-classpath scans with indexed queries while keeping JIndex independent of Minecraft-specific provenance.

## Considered options

- Keeping reference scanning in each consumer preserves the smallest JIndex format but repeats class-file parsing for every query and duplicates resolution rules.
- Persisting complete instructions retains maximum information but makes the file format, memory use, and query path pay for data that symbol search and Find Usages do not need.
- Persisting semantic references performs the work once and stores only the relationships required by queries.

## Consequences

The index format becomes explicitly versioned and generated snapshots from older formats must be rebuilt. Consumers supply and interpret provenance separately. The existing Companion reference scanner remains a differential test oracle during migration and is removed from the runtime path after parity is proven.
