# DigitalOcean deployment

Bootstrap files for running an Avalanche homeserver on a DigitalOcean
droplet (with Postgres co-located on the same box). Walkthrough lives in
[`docs/40-deployment.md`](../../../docs/40-deployment.md).

| File                  | Purpose                                                                |
|-----------------------|------------------------------------------------------------------------|
| `cloud-init.yaml`     | **Recommended.** One-shot bootstrap — paste into DO's "user data".     |
| `setup.sh`            | Older imperative bootstrap (operator finishes config by hand via SSH). |
| `avalanche.service`   | systemd unit for the homeserver binary (also embedded in cloud-init).  |
| `Caddyfile`           | Caddy reverse-proxy + auto-TLS template (also embedded in cloud-init). |
| `avalanche.env`       | Environment file template (also embedded in cloud-init).               |

The standalone copies of the service / Caddyfile / env file are kept for
direct reference and editing; the `cloud-init.yaml` embeds equivalent
content inline so it's a single self-contained file.

These files are also usable on Hetzner, Linode, or any Ubuntu 24.04 host —
they don't depend on DO-specific APIs beyond user-data (which is a
standard cloud-init feature).
