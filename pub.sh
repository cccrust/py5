set -x
cargo package

git add -A
git commit -m "$1"
git push

cargo publish
