#!/usr/bin/env sh
# Run the published image without cloning the repository.
set -eu

THEYWORK_IMAGE=${THEYWORK_IMAGE:-ghcr.io/giuseppecarte/they-work:latest}

if [ -n "${HOME:-}" ]; then
    THEYWORK_DEFAULT_CLAUDE_HOST="${HOME}/.claude"
    THEYWORK_DEFAULT_CODEX_HOST="${HOME}/.codex"
else
    THEYWORK_DEFAULT_CLAUDE_HOST=
    THEYWORK_DEFAULT_CODEX_HOST=
fi
THEYWORK_CLAUDE_HOST=${THEYWORK_CLAUDE_HOST:-$THEYWORK_DEFAULT_CLAUDE_HOST}
THEYWORK_CODEX_HOST=${THEYWORK_CODEX_HOST:-$THEYWORK_DEFAULT_CODEX_HOST}
THEYWORK_DOCKER_USER="$(id -u):$(id -g)"

echo "Pulling $THEYWORK_IMAGE ..." >&2
if THEYWORK_PULL_OUTPUT=$(docker pull "$THEYWORK_IMAGE" 2>&1); then
    printf '%s\n' "$THEYWORK_PULL_OUTPUT" >&2
else
    THEYWORK_PULL_STATUS=$?
    printf '%s\n' "$THEYWORK_PULL_OUTPUT" >&2
    case "$THEYWORK_PULL_OUTPUT" in
        *"manifest unknown"*|*"manifest not found"*|*"not found"*)
            echo "Image pull failed: image or tag not found: $THEYWORK_IMAGE" >&2 ;;
        *denied*|*unauthorized*)
            echo "Image pull failed: registry access denied for $THEYWORK_IMAGE. A denied response alone cannot distinguish an unpublished package from a restricted one; check the release run and package visibility." >&2 ;;
        *)
            echo "Image pull failed for $THEYWORK_IMAGE (Docker exit $THEYWORK_PULL_STATUS); see the Docker error above." >&2 ;;
    esac
    exit "$THEYWORK_PULL_STATUS"
fi

THEYWORK_NONINTERACTIVE=
case " $* " in
    *" --doctor "*|*" --once "*) THEYWORK_NONINTERACTIVE=1 ;;
esac

if [ -t 0 ]; then
    THEYWORK_TTY_INPUT=
elif [ -r /dev/tty ] && [ -t 1 ]; then
    # `curl ... | sh` pipes the script's stdin; reconnect the application to
    # the caller's terminal when one is available.
    THEYWORK_TTY_INPUT=/dev/tty
elif [ -z "$THEYWORK_NONINTERACTIVE" ]; then
    echo "an interactive terminal is required to run they-work" >&2
    exit 1
else
    THEYWORK_TTY_INPUT=
fi

if [ -n "$THEYWORK_NONINTERACTIVE" ]; then
    THEYWORK_DOCKER_TTY=
else
    THEYWORK_DOCKER_TTY=-it
fi

run_container() {
    if [ -n "$THEYWORK_TTY_INPUT" ]; then
        exec docker run --rm $THEYWORK_DOCKER_TTY \
            --user "$THEYWORK_DOCKER_USER" \
            --network none \
            --read-only \
            --cap-drop ALL \
            --security-opt no-new-privileges \
            -e TERM="${TERM:-xterm-256color}" \
            -e COLORTERM="${COLORTERM:-truecolor}" \
            -e TERM_PROGRAM \
            -e THEYWORK_CLAUDE_HOME=/data/claude \
            -e THEYWORK_CODEX_HOME=/data/codex \
            -e THEYWORK_ENCODING \
            -e THEYWORK_COLOR \
            -e NO_COLOR \
            "$@" <"$THEYWORK_TTY_INPUT"
    fi
    exec docker run --rm $THEYWORK_DOCKER_TTY \
        --user "$THEYWORK_DOCKER_USER" \
        --network none \
        --read-only \
        --cap-drop ALL \
        --security-opt no-new-privileges \
        -e TERM="${TERM:-xterm-256color}" \
        -e COLORTERM="${COLORTERM:-truecolor}" \
        -e TERM_PROGRAM \
        -e THEYWORK_CLAUDE_HOME=/data/claude \
        -e THEYWORK_CODEX_HOME=/data/codex \
        -e THEYWORK_ENCODING \
        -e THEYWORK_COLOR \
        -e NO_COLOR "$@"
}

run_with_both_homes() {
    run_container \
        -v "$THEYWORK_CLAUDE_HOST:/data/claude:ro" \
        -v "$THEYWORK_CODEX_HOST:/data/codex:ro" \
        "$THEYWORK_IMAGE" "$@"
}

run_with_claude_home() {
    run_container \
        -v "$THEYWORK_CLAUDE_HOST:/data/claude:ro" \
        "$THEYWORK_IMAGE" "$@"
}

run_with_codex_home() {
    run_container \
        -v "$THEYWORK_CODEX_HOST:/data/codex:ro" \
        "$THEYWORK_IMAGE" "$@"
}

run_without_agent_homes() {
    run_container \
        "$THEYWORK_IMAGE" "$@"
}

if [ -d "$THEYWORK_CLAUDE_HOST" ] && [ -d "$THEYWORK_CODEX_HOST" ]; then
    run_with_both_homes "$@"
elif [ -d "$THEYWORK_CLAUDE_HOST" ]; then
    echo "Codex home not found; continuing with Claude data only." >&2
    run_with_claude_home "$@"
elif [ -d "$THEYWORK_CODEX_HOST" ]; then
    echo "Claude home not found; continuing with Codex data only." >&2
    run_with_codex_home "$@"
else
    echo "No agent home found; live mode shows setup guidance. Use --demo to see the imaginary company." >&2
    run_without_agent_homes "$@"
fi
