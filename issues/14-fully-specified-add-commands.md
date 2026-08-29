# 14 - `AddBucket`/`AddJoint` do not name the node they create

Status: open
Severity: medium
Depends on: [02](02-pipe-timeout-mismatch.md)

## Problem

`ModifyCmd::AddBucket { parent }` and `ModifyCmd::AddJoint { parent }`
(`crates/bucket-spigot/src/lib.rs:532-545`) name a *parent* and let the network choose
the child index (`lib.rs:230-268`, `dest.push(child)`). Every other `ModifyCmd` variant
names the node it acts on:

| Command | Addresses | Re-applying the same command |
|---|---|---|
| `DeleteEmpty { path }` | the node | converges (second call: `UnknownPath`) |
| `FillBucket { bucket, .. }` | the node | same result |
| `SetFilters { path, .. }` | the node | same result |
| `SetWeight { path, .. }` | the node | same result |
| `SetOrderType { path, .. }` | the node | same result |
| `AddBucket { parent }` | the *parent* | **creates a second bucket** |
| `AddJoint { parent }` | the *parent* | **creates a second joint** |

The two outliers are the only commands whose meaning depends on state they do not
mention, and the only ones with a return value (`modify_and_get_created_path`,
`lib.rs:106-112`). That is exactly the corruption described in
[02](02-pipe-timeout-mismatch.md): the webui times out at 100 ms, the backend executes
anyway at 500 ms, the client retries, and the user gets two buckets.

## Why this fits the model

A `Network` *is* its command log. `ser.rs:160-220` serializes a network by emitting the
`ModifyCmd` sequence that rebuilds it (`add-joint .`, `add-bucket .0`, `set-weight …`);
the tree is the derived form. Under that model, `AddBucket { parent }` is an imperative
instruction ("append one more") embedded in what is otherwise a declarative log.

Making the add commands name their result turns *every* `ModifyCmd` into a total
assertion about the resulting state:

> `add-bucket .0.2` — after this command `.0.2` is a bucket, and it was not one before.

Consequences that fall out for free:

- **Replay is self-checking.** Deserializing a log into a differently-shaped network
  errors at the offending command instead of silently producing a different tree.
- **Creates stop having a return value.** The caller already knows the path it asked
  for, so a lost response costs nothing — this is what makes the retry safe, more than
  the duplicate-check does.
- **Retries are non-duplicating.** A retry hits "already exists" rather than appending.
- `modify_and_get_created_path` collapses to `modify`, and
  `ResponseData::NodeAdded { path }` (`beet-pusher/src/pipe_exec.rs`) becomes a bare ack.

## What this does *not* give

`Path` is a *location-dependent* identifier (`path.rs:8`), and `Path::modify_for_removed`
shifts sibling paths when a node is deleted. So:

- This is **at-most-once creation**, not true idempotence. A retry returns an error the
  client must interpret ("already exists at the path I asked for" is success-equivalent;
  every other error is not). The response type needs to make that distinction explicit
  rather than collapsing to a generic failure.
- There is an ABA hazard: delete `.0.2`, add a new node, and a stale retry of
  `add-bucket .0.2` "succeeds" against a different node. The single-writer event loop
  bounds this in practice, but it is real, and it is the reason a network-wide revision
  counter or a `RequestSequence` reply cache ([02](02-pipe-timeout-mismatch.md) fix 3)
  is still worth having as a complement, not an alternative.
- `BucketId` is the stable identity, but is not addressable in `ModifyCmd` — only
  readable via `find_bucket_path`. A `BucketId`-addressed create would be genuinely
  idempotent; it is a larger change and out of scope here. Noted as the eventual
  destination if positional addressing proves too fragile.

## Proposed change

Restrict the new path to the **next child index** (append-only). Arbitrary-index
insertion would require `order` and `bucket_paths` to renumber siblings, which is a
much larger change and is not needed for the retry-safety goal.

### 0. Migrate old imperative command to a different name (breaking)

NOTE: May move to deprecate the old imperative command later, at different interfaces

```rust
ModifyCmd::AddBucket { parent: Path } -> ModifyCmd::AddBucketTo { parent: Path }
ModifyCmd::AddJoint { parent: Path } -> ModifyCmd::AddJointTo { parent: Path }
```

The clap tests and tests using clap need to migrate.

### 1. Add canonical command form

```rust
ModifyCmd::AddBucket { new_path: Path }
ModifyCmd::AddJoint  { new_path: Path }
```

`Network::add_child` (`lib.rs:230`) already computes both halves it needs — it does
`split_last` on the requested path to get the parent, and compares the final index
against `dest.len()`. New `ModifyErr` variants:

- `AddPathOccupied(Path)` — index `< dest.len()`; the distinguishable "retry" case
- `AddPathNotNext { requested, expected }` — index `> dest.len()`; a gap
- existing `UnknownPath` / `AddToBucket` cover the missing- and invalid-parent cases

Note the existing `assert_eq!(child_index, child_index_expected)` in `add_child` becomes
a real check rather than an internal invariant assertion.

### 2. Resolution boundary for CLI/script convenience

`ModifyCmd` is stateless data; resolving "parent" to "parent + next index" needs the
network. Introduce an explicitly *unresolved* command at the clap/script layer:

```
clap::ModifyCmd::AddBucket { parent }   // convenience, resolved at apply time
  --(&Network)-->  ModifyCmd::AddBucket { new_path }   // canonical, checked
```

This keeps the CLI ergonomic (the non-idempotent convenience form stays where a human
is driving), keeps ~250 existing script call-sites parsing unchanged, and keeps the
strict form as the only thing on the wire.

The text serialization (`clap.rs:234-235`) needs a spelling for the strict form that the
loose form cannot be confused with, since both are one path argument. Suggested:
`add-bucket <parent> --index N`, back-compatible and unambiguous. `ser.rs` emits the
strict form; the parser accepts both.

### 3. Web API

Expose only the strict form. `SpigotCmd::AddNode { parent, node_kind }`
(`pipe_exec.rs:41`) becomes `AddNode { new_path, node_kind }`; `NodeService::create_bucket`
(`beet-pusher-webui/src/domain/services/node_service.rs:20`) takes the intended path
from the caller and maps `AddPathOccupied` at the requested path to success on retry.

The webui must then be able to *compute* the next path client-side, which means it needs
the current tree — check whether it has that today or whether a read endpoint is a
prerequisite. If it isn't available, this issue is blocked on it.

## Cost

Mostly mechanical, concentrated in `bucket-spigot`:

- `lib.rs` — `add_child` signature + validation, two `ModifyCmd` variants,
  `ModifyCmdRef` mirror, two `ModifyErr` variants
- `ser.rs:184-188` — emits `path` directly instead of `split_last`; **snapshot churn**
- `clap.rs` — `mirror_impl` entry, `Display`, new `--index` parsing
- `tests/arb_network.rs` — `NetworkGenerator` already owns the `Network` (line 176), so
  it can compute the next index; the `Seed` round-trip conversions need updating
- test scripts keep parsing via the resolution layer; snapshots that capture *serialized*
  output change (`tests/ser.rs`, and any `insta` snapshot of a network dump)
- downstream: `beet-pusher/src/pipe_exec.rs`, `beet-pusher-webui`, `spigot-visual`
  (`app/mod.rs:197-224`, plus regenerated TS bindings)

## Recommendation

Feasible and advisable. It removes an inconsistency that already exists independent of
the webui, and it makes the [02](02-pipe-timeout-mismatch.md) retry hazard structurally
impossible rather than timing-dependent. Do it after the timeout fix in
[02](02-pipe-timeout-mismatch.md) lands (which is the actual bug), and verify the webui
can obtain the tree before committing to step 3.
