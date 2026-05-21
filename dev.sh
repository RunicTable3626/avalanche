#!/usr/bin/env bash
set -e

echo "Starting Postgres..."
docker compose -f infra/docker-compose.yml up -d

echo "Waiting for Postgres to be healthy..."
until docker compose -f infra/docker-compose.yml ps postgres | grep -q "healthy"; do
  sleep 1
done

echo "Starting server and relay..."
# TODO: also start first-party Project services once Stage 6 lands
trap 'kill $(jobs -p)' EXIT
cd core
RUST_LOG=tower_http=debug,server=debug cargo run -p server &
RUST_LOG=relay=debug cargo run -p relay &
wait
