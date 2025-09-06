Operations Guide

Overview

- Roles: Validators, Gateways, Clients
- Traffic: Clients → Gateways (REST/WS), Gateways → Validators (consensus)
- Randomness: Commit‑Reveal among validators or VRF fallback

Day‑1 Tasks

- Configure `--role`, `--listen-tcp`, `--regions`, `--region-self`
- Gateways: set `lb_strategy` and `X-Region` at L7 if available
- Validators: tune PBFT batch/pipeline via CLI flags

Runbooks

- Regional Failover: mark region unhealthy, validate LB drain, monitor backlog
- Validator Rotation: add new, observe QC signers, remove failing validator
- Snapshot Recovery: fetch latest snapshot + QC, replay deltas, verify state hash

Monitoring

- Dashboard: `grafana/dashboards/bitcraps_operations.json`
- Key metrics: bet ingress→commit latency, commit→broadcast, WS backlog, view changes

Security

- Admin endpoints require API key/JWT
- Rate limits per IP/user; micro‑fees optional

