# Every zoned value the host creates is released. A handle released on the
# success path but not the failure path leaked silently: `with_time_zone!` with
# an invalid zone kept its value alive (D-T2-14), found only by a reviewer's
# trace. This builds an app that takes both paths through every zoned host
# call, runs it with the resource trace on, and requires as many releases as
# creations. (The trace's `miss` lines are ordinary Roc frees — strings, lists —
# passing the same hook, not resources.)
set -euo pipefail
mkdir -p "$TMP/app/app"
printf '[world]\nname = "app"\n\n[deps]\n%s' "$DEPS" > "$TMP/app/world.toml"
cp app.roc "$TMP/app/app/main.roc"
if ! out=$("$TRANTOR" build "$TMP/app" --app app --out leaks 2>&1); then
	echo "FAIL: build the leak app" >&2; echo "$out" | tail -20 >&2; exit 1
fi
trace="$TMP/trace"
stdout=$(TRANTOR_RESOURCE_TRACE=1 "$TMP/app/target/trantor/app/bin/leaks" 2> "$trace")
want="xxoxo oxoxooxo oooooooooo oooxoxoxx oxooxxooo | xxoxo oxoxooxo oooooooooo oooxoxoxx oxooxxooo"
[[ "$stdout" == "$want" ]] || { echo "FAIL: the calls took other paths than written: '$stdout', want '$want'"; exit 1; }
created=$(grep -c '^\[resource\] new:' "$trace" || true)
released=$(grep -c '^\[resource\] dealloc: .* HIT$' "$trace" || true)
[[ "$created" -gt 0 ]] || { echo "FAIL: the trace recorded no resources — is TRANTOR_RESOURCE_TRACE still honoured?"; exit 1; }
[[ "$created" == "$released" ]] || {
	echo "FAIL: $created zoned values created, $released released"; exit 1; }
echo "ok: tests/leaks — $created zoned values created on every host path, all released"
