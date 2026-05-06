set -x

sed -i '' "s/^version = \".*\"/version = \"$1\"/" Cargo.toml
git add -A
git commit -m "v$1"
git push

cargo package
cargo publish
