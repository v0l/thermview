#!/bin/sh
set -e
cd "$(dirname "$0")/.."
./web/build.sh
bunx wrangler pages deploy target/web --project-name webtherm --branch master --commit-dirty=true
