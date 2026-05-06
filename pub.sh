set -x

git add -A
git commit -m "$1"
git push

cargo package
cargo publish
