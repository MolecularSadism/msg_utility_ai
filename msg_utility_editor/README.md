# msg_utility_editor

Standalone node-graph editor for [`msg_utility_ai`](../msg_utility_ai) scoring trees. It edits and
simulates `UtilityAIProfile` assets directly — the `.ai.ron`/`.ai.json` files actors load — without
running the game.

Run it with:

```bash
cargo run
```

(from this directory — see [Standalone workspace](#standalone-workspace) below).

## Graph model

A profile is a graph of [`EditorNode`](src/graph/mod.rs)s connected through `egui-snarl` pins.
Converting between the graph and `UtilityAIProfile` (loading, saving, simulating) goes through
[`graph_to_profile`/`profile_to_graph`](src/graph/convert.rs).

| Group | Nodes | Purpose |
|---|---|---|
| Logic | `MUL`, `MAX`, `AVG`, `GEOM`, `NOT` | Combine or invert child scores |
| Transform | `Weighted`, `Curve`, `Clamp`, `Normalize` | Reshape a single input's value |
| Sources | `Consider`, `Fixed` | Read a named consideration (0.0-1.0 sensor input) or a constant |
| Output | `Composite`, `Behavior` | Name a reusable sub-score, or terminate a scoring chain into an action |

`Consider` nodes name the considerations the simulation panel exposes as sliders; `Composite`
nodes name intermediate scores other parts of the graph can reference by that name instead of
duplicating the subtree. A `Behavior` node's input is one "thought" toward that action — multiple
`Behavior` nodes sharing a name are grouped and scored together, and the highest-scoring group wins
according to the document's `PickStrategy` and threshold.

## Panels

- **Toolbar** (top) — File menu (New/Open/Save/Save As, `.ron` and `.json`), Node menu (insert any
  node type above), View menu (reset the force-directed layout)
- **Graph** (center) — the node canvas itself; drag to connect pins, right-click/menu to add nodes
- **Properties** (right) — inspector for the selected node's fields (weight, curve parameters,
  consideration/composite/behavior names, ...)
- **Simulation** (right, below Properties) — one 0.0-1.0 slider per `Consider` node; scores
  recompute live as sliders move, with the winning behavior highlighted in the graph

## File I/O

[`io::load`/`io::save`](src/io.rs) pick RON or JSON by file extension; both round-trip through
`UtilityAIProfile`'s `serde` impl, so a profile saved here loads unmodified through the game's
`.ai.ron` folder loader.

## Standalone workspace

This crate declares its own `[workspace]` so it never joins `msg_utility_ai`'s build graph or shares
its `target/`/lockfile — the repo root's workspace members list does not include it. Build and run
it the same way as any other standalone Rust project, from inside `msg_utility_editor/`:

```bash
cargo run      # debug build
cargo build --release
```

It depends on `msg_utility_ai` by path, so editing that crate is picked up on the next rebuild here
without publishing anything.
