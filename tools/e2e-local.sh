#!/usr/bin/env bash
# 로컬 end-to-end 확인: wrangler dev 로 signaling 을 띄우고, host-agent 하나에
# 틀린 코드로 한 번, 맞는 코드로 한 번 viewer 를 붙인다. 마지막 줄이 `E2E OK` 면 통과.
# 실행: 저장소 루트에서 `bash tools/e2e-local.sh` (WSL, ~/sr-env.sh 를 먼저 source).
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT=$(pwd)
BIN="${CARGO_TARGET_DIR:-$ROOT/target}/debug"
SERVER=ws://127.0.0.1:8787
export WRANGLER_SEND_METRICS=false

WRANGLER_PID=""
H_PID=""
CAT_PID=""
WORK=$(mktemp -d)

cleanup() {
    local status=$?
    set +m  # 끝낸 작업마다 "Terminated" 를 알리지 않게 한다.
    # 자식의 자식(wrangler 가 띄운 workerd 등)까지 process group 으로 끝낸다.
    local groups="$H_PID $WRANGLER_PID"
    for pid in $groups; do
        kill -- "-$pid" 2>/dev/null || true
    done
    # wrangler 는 SIGTERM 뒤 정리에 몇 초 걸린다. 최대 10초 기다리고 남으면 SIGKILL.
    for _ in $(seq 1 50); do
        local alive=""
        for pid in $groups; do
            kill -0 -- "-$pid" 2>/dev/null && alive=1
        done
        [ -z "$alive" ] && break
        sleep 0.2
    done
    for pid in $groups; do
        kill -KILL -- "-$pid" 2>/dev/null || true
    done
    wait 2>/dev/null || true
    if [ "$status" -ne 0 ]; then
        echo "--- wrangler log ---" >&2
        cat "$WORK/wrangler.log" >&2 2>/dev/null || true
        echo "--- host-agent stderr ---" >&2
        cat "$WORK/host.err" >&2 2>/dev/null || true
        echo "E2E FAILED" >&2
    fi
    rm -rf "$WORK"
    exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT TERM

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

# job control 을 켜면 background 작업마다 process group 이 따로 생겨 한꺼번에 끝낼 수 있다.
set -m

echo "== build"
cargo build -p host-agent -p viewer

echo "== start signaling (wrangler dev)"
(cd signaling && exec npx wrangler dev --port 8787 --ip 127.0.0.1 --show-interactive-dev-session=false) \
    >"$WORK/wrangler.log" 2>&1 &
WRANGLER_PID=$!
for _ in $(seq 1 30); do
    code=$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8787/ || true)
    [ "$code" = "404" ] && break
    sleep 1
done
[ "$code" = "404" ] || fail "signaling did not answer 404 within 30s (last: $code)"

echo "== start host-agent"
coproc HOST { set +o pipefail; yes y | "$BIN/host-agent" --server "$SERVER" --name e2e-host --once 2>"$WORK/host.err"; }
H_PID=$HOST_PID
# coproc 의 fd 는 background 작업에 넘어가지 않으므로 복사해 둔다.
exec {HOST_OUT}<&"${HOST[0]}"

ID=""
CODE=""
while [ -z "$ID" ] || [ -z "$CODE" ]; do
    IFS= read -r -t 60 line <&"$HOST_OUT" || fail "host-agent printed no ID/CODE within 60s"
    echo "host: ${line%% *} ..."
    case "$line" in
        "ID "*) ID=${line#ID } ;;
        "CODE "*) CODE=${line#CODE } ;;
    esac
done
[[ "$ID" =~ ^[1-9][0-9]{8}$ ]] || fail "bad ID line: $ID"
[[ "$CODE" =~ ^[0-9]{6}$ ]] || fail "bad CODE line"
echo "host registered as $ID"

# host 의 stdout 을 계속 비워 host 가 pipe 에 막히지 않게 한다.
cat <&"$HOST_OUT" >"$WORK/host.out" &
CAT_PID=$!

echo "== viewer with a wrong code"
WRONG=000000
[ "$CODE" = "$WRONG" ] && WRONG=111111
set +e
timeout 60 "$BIN/viewer" --server "$SERVER" --id "$ID" --code "$WRONG" --name e2e-viewer \
    >"$WORK/v1.out" 2>"$WORK/v1.err"
st=$?
set -e
cat "$WORK/v1.out" "$WORK/v1.err"
[ "$st" -eq 1 ] || fail "wrong code: expected exit 1, got $st"
grep -q "코드가 틀렸습니다" "$WORK/v1.err" || fail "wrong code: message missing"

# 한 번 틀리면 host 는 1초 기다리게 한다.
sleep 2

echo "== viewer with the right code"
set +e
timeout 90 "$BIN/viewer" --server "$SERVER" --id "$ID" --code "$CODE" --name e2e-viewer \
    >"$WORK/v2.out" 2>"$WORK/v2.err"
st=$?
set -e
cat "$WORK/v2.out" "$WORK/v2.err"
[ "$st" -eq 0 ] || fail "right code: expected exit 0, got $st"
grep -q "^CONNECTED e2e-host " "$WORK/v2.out" || fail "right code: CONNECTED line missing"
[ "$(grep -c '^PONG rtt_ms=[0-9]*$' "$WORK/v2.out")" -eq 3 ] || fail "right code: expected 3 PONG lines"
if grep -q "$CODE" "$WORK/v1.out" "$WORK/v1.err" "$WORK/v2.out" "$WORK/v2.err"; then
    fail "viewer printed the code"
fi

echo "== host-agent --once exits after the session"
for _ in $(seq 1 60); do
    kill -0 "$H_PID" 2>/dev/null || break
    sleep 1
done
kill -0 "$H_PID" 2>/dev/null && fail "host-agent still running 60s after the session"
set +e
wait "$H_PID"
st=$?
set -e
H_PID=""
wait "$CAT_PID"  # host 의 출력을 끝까지 옮겨 적을 때까지
sed 's/^CODE .*/CODE ******/' "$WORK/host.out"
[ "$st" -eq 0 ] || fail "host-agent exit status $st"
# 틀린 코드의 viewer 는 host 의 mac_b 로 틀린 것을 알고 확인 MAC 없이 떠나므로 host 에는 NoConfirm 으로 남는다.
grep -Eq "^ATTEMPT (NoConfirm|WrongCode)$" "$WORK/host.out" || fail "host did not report the failed attempt"

echo "E2E OK"
