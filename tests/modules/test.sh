# Nothing this package writes into a world's composed platform directory may
# claim a generic name. A component's `exports` land there as `<Module>.roc`
# whether or not `[package] exports` mentions them, and trantor treats a second
# source for one platform module name as a hard compose error — so a private
# module called `Plain` or `Strftime` took those names away from every other
# package in the world, and `[package] exports` said nothing about it. Both
# were exactly that until they became `TemporalPlain` and `TemporalStrftime`.
#
# The rule checked here is the ecosystem's: every module this package adds to a
# platform carries one of the prefixes its exported surface is named for. That
# still admits a new `TemporalX` without editing this test, and refuses a new
# `Plain`. Read by composing twice — with the package and with its dev-deps
# alone — because what the package adds is the difference between the two.
set -euo pipefail

PREFIXES='^(Temporal|Now)'

# A world at $TMP/<name>, composed; prints its platform's module names, sorted.
platform_modules() {
	name=$1
	deps=$2
	mkdir -p "$TMP/$name/app"
	printf '[world]\nname = "%s"\n\n[deps]\n%s' "$name" "$deps" > "$TMP/$name/world.toml"
	if ! out=$("$TRANTOR" compose "$TMP/$name" 2>&1); then
		echo "FAIL: compose the '$name' world" >&2; echo "$out" | tail -20 >&2; exit 1
	fi
	ls "$TMP/$name/target/trantor/$name/platform" | grep '\.roc$' | LC_ALL=C sort
}

platform_modules with "$DEPS" > "$TMP/with.txt"
platform_modules without "$DEV_DEPS" > "$TMP/without.txt"

added=$(comm -13 "$TMP/without.txt" "$TMP/with.txt")
[[ -n "$added" ]] || {
	echo "FAIL: the package added no platform module at all — the two worlds composed the same thing"; exit 1; }

count=0
generic=""
names=""
for m in $added; do
	count=$((count + 1))
	names="$names ${m%.roc}"
	[[ "$m" =~ $PREFIXES ]] || generic="$generic $m"
done
[[ -z "$generic" ]] || {
	echo "FAIL:$generic in the composed platform directory — a module this package writes there must carry its prefix, or it takes that name from every other package in the world"
	exit 1; }

echo "ok: tests/modules — $count modules added to a composed platform, all prefixed:$names"
