# 13 - Revisit the ports-and-adapters layering in `beet-pusher-webui`

Status: deferred — revisit once 3-4 endpoints exist
Severity: n/a (design)

## Context

`beet-pusher-webui` is laid out in the ports-and-adapters ("hexagonal") style:

```
api/          driving adapter   — axum handlers, DTOs, extractors
domain/       the core          — services + the ports they depend on
infra/        driven adapter    — StdioPipe, talks to beet-pusher
state.rs      composition       — AppState<T> wires them together
```

The review called this "hexagonal-ish". This file explains the qualifier and what would
resolve it. It is filed as *deferred* deliberately: with one endpoint in place there is
not yet enough evidence to know which way to go.

## What the layering gets right

- **`BeetPusherPipe` is a real port** (`domain/services/ports.rs`). It is defined by the
  core, not by the adapter, and `infra::StdioPipe` implements it. That is the correct
  dependency direction — the arrow points inward.
- **The port pays for itself in tests.** `AppState<T>` is generic over the pipe, so
  `tests/entrypoint.rs` substitutes a `TestPipe` and exercises every handler with no
  subprocess at all. This is the main practical benefit of the pattern and it is being
  collected.
- **DTOs are separate from domain types.** `CreateBucketDto` / `CreateBucketResponse`
  carry `String`, not `NodePath` (commit `2137ea6`, "remove domain types from dtos"), so
  the wire format can evolve independently of the core.
- **Errors flow outward correctly.** `error.rs` knows about
  `node_service::CreateBucketError` and maps it to HTTP status codes; the domain knows
  nothing about HTTP.

## Where it deviates

**1. The port speaks the adapter's vocabulary, not the domain's.**

```rust
// domain/services/ports.rs
fn send(&self, command: beet_pusher::pipe_exec::Command, timeout: Duration)
    -> impl Future<Output = Result<ResponseData, Self::Error>>;
```

`Command` and `ResponseData` are `beet-pusher`'s *wire protocol* types. So the domain
core depends directly on the serialisation format of the process behind the port — which
is the coupling the port was introduced to prevent. A port in the intended sense would
express intent rather than transport:

```rust
trait NodeStore {
    async fn add_bucket(&self, parent: NodePath) -> Result<NodePath, AddBucketError>;
}
```

with all `pipe_exec` types confined to `infra::StdioPipe`. Change the wire protocol and
only the adapter moves.

**2. The domain service is doing adapter work.** `NodeService::create_bucket`
(`domain/services/node_service.rs:19-39`) builds a protocol message, sends it, and
pattern-matches the response variant. That is translation, not domain logic — and there
is no domain logic here yet, so the layer is currently pure indirection: a hop from
handler to service to port, with each hop just reshaping the same request.

**3. An infrastructure concern sits in the core.** `const TIMEOUT = 100ms`
(`node_service.rs:6`) is a transport property living in a domain service, which is also
how it ended up mismatched with the backend's timeout — see
[02](02-pipe-timeout-mismatch.md). It belongs in the adapter or in config.

**4. The generic propagates widely.** `T: BeetPusherPipe` threads through `AppState<T>`,
`Router<AppState<T>>`, every route constructor, and every handler signature. This is the
price of static dispatch. `Arc<dyn BeetPusherPipe>` would collapse it, but the trait uses
`async fn` in trait position, so dyn-compatibility needs either boxed futures
(`-> Pin<Box<dyn Future>>`) or the `async-trait` crate. Worth weighing once there are
enough handlers to feel the cost.

## The real question, for later

The pattern's cost is per-endpoint (three files touched to add one route); its benefit is
per-*substitution* (swapping the pipe in tests, and one day perhaps an in-process backend
rather than a subprocess). One endpoint is not enough data.

Concretely, revisit after 3-4 endpoints exist and ask:

- Has any *domain logic* accumulated in `domain/services/` — validation spanning
  multiple fields, ordering rules, state the core owns? **If yes**, the layer is earning
  its keep: finish the job by fixing deviation (1) so the core stops depending on the
  wire format.
- Or is every service method still a three-line translate-and-forward? **If so**, the
  honest simplification is to delete `domain/services/` and have handlers call the port
  directly, keeping `BeetPusherPipe` purely as the test seam — which is where the
  value actually is.

Either answer is fine. The thing to avoid is drifting into the shape of the pattern
without deciding which one is true.

## Smaller related note

Validation is currently split: `ValidatedJson` + `validator` handle a schema that
declares no rules, while the real validation (`parent.parse::<NodePath>()`) happens
inline in the handler and produces `AppError::Validation`. Deciding where validation
lives is part of the same question. See also [12](12-dependency-surface-growth.md).
