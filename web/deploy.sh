#!/bin/sh
set -e
cd "$(dirname "$0")"
./build.sh
rm -rf dist
mkdir -p dist
cp index.html dist/
cp -r pkg dist/pkg
rm -f dist/pkg/.gitignore
bunx wrangler pages deploy dist --project-name=wire-antenna-calc --branch=main --commit-dirty=true
