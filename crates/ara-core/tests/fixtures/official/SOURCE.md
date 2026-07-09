# Official ARA fixtures — source & attribution

The two artifacts under this directory are **verbatim, pinned copies** of the
canonical examples from the Agent-Native Research Artifact project. They are the
frozen corpus `ara-core` is parsed against; do not hand-edit them (regenerate by
re-copying from the pinned commit below).

- **Upstream repo:** https://github.com/ARA-Labs/Agent-Native-Research-Artifact
- **Pinned commit:** `c6366a9eb4d79ad6e5179f0aea1a59349e9ff09f`
- **License:** MIT (see the upstream `LICENSE`). Note: the Stage-1 plan assumed
  MPL-2.0, but the upstream repository is MIT-licensed; this file records the
  actual license.

## Files

| Fixture | Copied from (upstream path) |
|---------|-----------------------------|
| `minimal-artifact/trace/exploration_tree.yaml` | `examples/minimal-artifact/trace/exploration_tree.yaml` |
| `minimal-artifact/logic/claims.md` | `examples/minimal-artifact/logic/claims.md` |
| `resnet-ara-example/trace/exploration_tree.yaml` | `examples/resnet-ara-example/trace/exploration_tree.yaml` |
| `resnet-ara-example/logic/claims.md` | `examples/resnet-ara-example/logic/claims.md` |

Only `trace/exploration_tree.yaml` and `logic/claims.md` are copied — the parser
consumes only those two files in Stage 1.
