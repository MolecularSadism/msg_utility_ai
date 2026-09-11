# msg_utility_editor

A utility AI evaluation library and its standalone node-graph profile editor, extracted from
[gravitaria](https://github.com/ffmulks/KUGELBLITZ) for reuse across MolecularSadism projects.

| Crate | Purpose |
|---|---|
| [`msg_utility_ai`](msg_utility_ai) | Utility AI evaluation library with data-driven scoring trees |
| [`msg_utility_editor`](msg_utility_editor) | Node-graph editor for `msg_utility_ai` scoring trees (standalone tool, own workspace) |

## Using `msg_utility_ai` in another project

```toml
msg_utility_ai = { git = "https://github.com/MolecularSadism/msg_utility_ai", tag = "v0.1.0" }
```

`msg_utility_editor` is a dev tool only — it is not meant to be depended on.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
