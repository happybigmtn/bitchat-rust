Scale Testing Infrastructure

Goals

- Validate 100k+ concurrent spectators, realistic bet ingress, and WS fan‑out

Components

- Example load tool: `examples/gateway_load.rs`
  - Usage: `cargo run --example gateway_load -- <base_url> <game_id_hex> <concurrency> <requests_per_client> <player_prefix>`
  - Sends batched `/api/v1/games/:id/bets` requests

Scenarios

- Steady state: 10k clients × 10 rps → 100k rpm
- Burst: 20k clients spike for 30s; monitor rate limits and latency

Metrics to Watch

- `bet_ingress_latency_ms` p95/p99, `commit_broadcast_latency_ms` p95/p99
- `ws_topic_backlog{shard}`, `lb_circuit_state`, `consensus_view_changes_total`

Tuning

- Increase `pbft_batch_size`, `pbft_pipeline_depth` for validators
- Adjust `lb_strategy` and `X-Region` routing for gateways

