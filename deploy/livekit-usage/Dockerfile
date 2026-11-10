FROM golang:1.26-bookworm AS build
ARG LIVEKIT_COMMIT=385aa40f9292fe1db8caad87301e4b285b7c198a
RUN git clone --filter=blob:none --no-checkout https://github.com/livekit/livekit.git /src \
    && cd /src && git checkout "$LIVEKIT_COMMIT"
WORKDIR /src
COPY deploy/livekit-usage/stats.patch /tmp/stats.patch
RUN git apply --check /tmp/stats.patch && git apply /tmp/stats.patch
COPY deploy/livekit-usage/usage_metrics.go pkg/telemetry/usage_metrics.go
RUN CGO_ENABLED=0 go build -trimpath -o /livekit-server ./cmd/server

FROM livekit/livekit-server:v1.13.11
COPY --from=build /livekit-server /livekit-server
