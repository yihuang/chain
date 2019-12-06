#!/bin/sh
rm -r vendor
mkdir vendor
cp Cargo.* ./vendor
ls -1 */src/lib.rs | xargs -I{} rsync -R {} ./vendor
ls -1 */src/main.rs | xargs -I{} rsync -R {} ./vendor
ls -1 */Cargo.toml | xargs -I{} rsync -R {} ./vendor
find ./vendor -name "lib.rs" | xargs -I{} sh -c 'echo > {}'
find ./vendor -name "main.rs" | xargs -I{} sh -c 'echo "fn main() { }" > {}'

docker build -f docker/Dockerfile_vendor -t crypto-chain-vendor . && rm -r ./vendor
