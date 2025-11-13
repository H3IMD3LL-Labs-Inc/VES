#!/usr/bin/bash

# -------------------- CONFIG ---------------------
LOG_DIR="/var/log/testlogs"
THROUGHPUT_PER_SECOND=1000000              # adjust to stress test throughput
BURST_PROBABILITY=0.1                       # probability of 0-1 for a short high-rate burst
SLEEP_BASE_MICROS=100                       # base delay between log writes in micro-seconds

mkdir -p "$LOG_DIR"

# Target files per format
CRI_LOG="$LOG_DIR/cri.log"
DOCKER_JSON_LOG="$LOG_DIR/docker_json.log"
ARBITRARY_JSON_LOG="$LOG_DIR/arbitrary_json.log"
SYSLOG_RFC3164_LOG="$LOG_DIR/syslog_3164.log"
SYSLOG_RFC5424_LOG="$LOG_DIR/syslog_5424.log"

# Timestamp helpers
iso8601() { date -u +"%Y-%m-%dT%H:%M:%S.%3NZ"; }
rfc3164_ts() { date -u +"%b %d %H:%M:%S"; }

# -------------------- GENERATORS -------------------
generate_cri() {
    echo "$(iso8601) stdout F Hello from CRI log PID=$RANDOM LEVEL=INFO"
}

generate_docker_json() {
    printf '{"log":"Processing event ID=%d,"stream":"stdout","time":"%s"}\n' "$RANDOM" "$(iso8601)"
}

generate_arbitrary_json() {
    printf '{"level":"%s","component":"%s","msg":"event id=%d occured","timestamp":"%s"}\n' \
        "$(shuf -e INFO WARN ERROR DEBUG | head -1)" \
        "$(shuf -e auth net db fs io | head -1)" \
        "$RANDOM" "$(iso8601)"
}

generate_syslog_rfc3164() {
    echo "<34>$(rfc3164_ts) testhost app[123]: Sample RFC3164 syslog message $RANDOM"
}

generate_syslog_rfc5424() {
    echo "<165>1 $(iso8601) testhost app 1234 ID47 [exampleSDID@32473 iut\"3\" eventSource=\"app\" eventID=\"$RANDOM\"] Sample RFC5424 syslog"
}

# -------------------- MAIN LOOP -------------------
echo "Generating mixed-format logs in $LOG_DIR..."
echo "Target log generation rate: ~$THROUGHPUT_PER_SECOND logs/sec"
echo "Press CTRL+C to stop."

while true; do
    # Random burst mode to simulate throughput variation
    if (( $(awk -v r=$RANDOM -v p=$BURST_PROBABILITY 'BEGIN {print (r/32767 < p)}') )); then
        RATE=$((THROUGHPUT_PER_SECOND * 2))
    else
        RATE=$THROUGHPUT_PER_SECOND
    fi

    for ((i=0; i<RATE/5; i++)); do
        generate_cri >> "$CRI_LOG" &
        generate_docker_json >> "$DOCKER_JSON_LOG" &
        generate_arbitrary_json >> "$ARBITRARY_JSON_LOG" &
        generate_syslog_rfc3164 >> "$SYSLOG_RFC3164_LOG" &
        generate_syslog_rfc5424 >> "$SYSLOG_RFC5424_LOG" &
    done

    # Sleep to control rate (roughly)
    usleep "$SLEEP_BASE_MICROS"
done
