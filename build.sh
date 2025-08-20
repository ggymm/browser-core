#!/bin/sh

npm run build

files=(
  "index.js"
  "index.d.ts"
  "browser-core.darwin-arm64.node"
)
mv "${files[@]}" dist
