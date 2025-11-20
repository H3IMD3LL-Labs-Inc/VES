#!/usr/bin/env bash

# -------------------- CONFIG ---------------------
PID_FILE="/tmp/generate_test_logs.pid"
LOG_DIR="/var/log/testlogs/actuallogs"
GENERATOR_LOG="/var/log/testlogs/generator.log"
MAX_LOG_SIZE_MB=100                  # Rotate log after this size
MAX_JOBS=20                          # Limit concurrent background jobs
THROUGHPUT_PER_SECOND=500000        # Base log generation rate
BURST_PROBABILITY=0.1
SLEEP_BASE_MICROS=500

mkdir -p "$LOG_DIR"

# -------------------- COLORS ---------------------
info()    { echo -e "\e[34m[INFO]\e[0m $*"; }
success() { echo -e "\e[32m[SUCCESS]\e[0m $*"; }
warn()    { echo -e "\e[33m[WARN]\e[0m $*"; }
err()     { echo -e "\e[31m[ERROR]\e[0m $*"; }

# -------------------- LOG FILES ---------------------
CRI_LOG="$LOG_DIR/cri.log"
DOCKER_JSON_LOG="$LOG_DIR/docker_json.log"
ARBITRARY_JSON_LOG="$LOG_DIR/arbitrary_json.log"
SYSLOG_RFC3164_LOG="$LOG_DIR/syslog_3164.log"
SYSLOG_RFC5424_LOG="$LOG_DIR/syslog_5424.log"

# -------------------- TIMESTAMP HELPERS ---------------------
# Nanosecond precision (required for CRI, Docker, JSON log formats)
iso8601() { date -u +"%Y-%m-%dT%H:%M:%S.%NZ"; }

rfc3164_ts() { date -u +"%b %d %H:%M:%S"; }

# -------------------- LOG ROTATION ---------------------
rotate_log() {
    local file=$1
    if [[ -f "$file" ]]; then
        local size_mb
        size_mb=$(du -m "$file" | cut -f1)
        if (( size_mb >= MAX_LOG_SIZE_MB )); then
            mv "$file" "$file.$(date +%Y%m%d%H%M%S)"
            touch "$file"
            info "Rotated log $file"
        fi
    fi
}

# -------------------- GENERATORS -------------------
generate_cri() {
    echo "$(iso8601) stdout F Hello from CRI log PID=$RANDOM LEVEL=INFO"
}

generate_docker_json() {
    printf '{"log":"Processing event ID=%d","stream":"stdout","time":"%s"}\n' \
        "$RANDOM" "$(iso8601)"
}

generate_arbitrary_json() {
    printf '{"time":"%s","level":"%s","msg":"event id=%d occurred"}\n' \
        "$(iso8601)" \
        "$(shuf -e INFO WARN ERROR DEBUG | head -1)" \
        "$RANDOM"
}

generate_syslog_rfc3164() {
    echo "<34>$(rfc3164_ts) testhost app[123]: Sample RFC3164 syslog message $RANDOM"
}

generate_syslog_rfc5424() {
    echo "<165>1 $(iso8601) testhost app 1234 ID47 [exampleSDID@32473 iut=\"3\" eventSource=\"app\" eventID=\"$RANDOM\"] Sample RFC5424 syslog"
}

# -------------------- MAIN GENERATOR LOOP -------------------
run_generator() {
    info "Generating mixed-format logs in $LOG_DIR..."
    info "Target log rate: ~$THROUGHPUT_PER_SECOND logs/sec"

    while true; do
        # Burst mode
        if (( $(awk -v r=$RANDOM -v p=$BURST_PROBABILITY 'BEGIN {print (r/32767 < p)}') )); then
            RATE=$((THROUGHPUT_PER_SECOND * 2))
        else
            RATE=$THROUGHPUT_PER_SECOND
        fi

        JOBS=0
        for ((i=0; i<RATE/5; i++)); do
            generate_cri >> "$CRI_LOG" &
            generate_docker_json >> "$DOCKER_JSON_LOG" &
            generate_arbitrary_json >> "$ARBITRARY_JSON_LOG" &
            generate_syslog_rfc3164 >> "$SYSLOG_RFC3164_LOG" &
            generate_syslog_rfc5424 >> "$SYSLOG_RFC5424_LOG" &
            JOBS=$((JOBS + 5))

            # Limit concurrent background jobs
            if (( JOBS >= MAX_JOBS )); then
                wait
                JOBS=0
            fi
        done

        # Rotate logs if needed
        rotate_log "$CRI_LOG"
        rotate_log "$DOCKER_JSON_LOG"
        rotate_log "$ARBITRARY_JSON_LOG"
        rotate_log "$SYSLOG_RFC3164_LOG"
        rotate_log "$SYSLOG_RFC5424_LOG"
        rotate_log "$GENERATOR_LOG"

        usleep "$SLEEP_BASE_MICROS"
    done
}

# -------------------- CONTROL FUNCTIONS -------------------
start_generator() {
    if [[ -f "$PID_FILE" ]] && kill -0 "$(cat "$PID_FILE")" 2>/dev/null; then
        warn "Already running (PID $(cat "$PID_FILE"))"
        exit 0
    fi

    info "Starting log generator..."
    nohup bash "$0" run >>"$GENERATOR_LOG" 2>&1 &
    NEWPID=$!
    sleep 0.3

    if kill -0 "$NEWPID" 2>/dev/null; then
        echo "$NEWPID" > "$PID_FILE"
        success "Started (PID $NEWPID)"
    else
        err "Failed to start generator"
    fi
}

stop_generator() {
    if [[ ! -f "$PID_FILE" ]]; then
        warn "Not running (no PID file)"
        exit 0
    fi

    PID=$(cat "$PID_FILE")
    info "Stopping PID $PID..."
    if kill "$PID" 2>/dev/null; then
        rm -f "$PID_FILE"
        success "Stopped log generator"
    else
        err "Failed to stop — process missing"
        rm -f "$PID_FILE"
    fi
}

status_generator() {
    if [[ ! -f "$PID_FILE" ]]; then
        warn "Not running"
        exit 0
    fi

    PID=$(cat "$PID_FILE")
    if kill -0 "$PID" 2>/dev/null; then
        success "Running (PID $PID)"
    else
        err "PID file exists but process dead"
    fi
}

# -------------------- DISPATCH -------------------
case "$1" in
    start)  start_generator ;;
    stop)   stop_generator ;;
    status) status_generator ;;
    run)    run_generator ;;
    *) echo "Usage: $0 {start|stop|status}"; exit 1 ;;
esac
