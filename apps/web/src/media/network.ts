/** Only aggregate route facts leave RTC reports. Never return addresses, URLs or credentials. */
export function networkSummary(stats: RTCStatsReport | undefined): {
  route: string;
  rttMs: number | null;
} {
  let selected: string | undefined;
  stats?.forEach((stat) => {
    if (stat.type === "transport" && stat.selectedCandidatePairId)
      selected = stat.selectedCandidatePairId;
  });
  let pair = selected ? stats?.get(selected) : undefined;
  if (!pair)
    stats?.forEach((stat) => {
      if (
        stat.type === "candidate-pair" &&
        stat.state === "succeeded" &&
        (stat.selected || stat.nominated)
      )
        pair = stat;
    });
  const local = pair && stats?.get(pair.localCandidateId);
  const route =
    local?.relayProtocol === "tls"
      ? "TURN/TLS"
      : local?.candidateType === "relay"
        ? local.relayProtocol === "tcp"
          ? "TURN/TCP"
          : local.relayProtocol === "udp"
            ? "TURN/UDP"
            : "TURN"
        : local?.protocol === "tcp"
          ? "TCP"
          : local?.protocol === "udp"
            ? "UDP"
            : "unknown";
  return {
    route,
    rttMs:
      typeof pair?.currentRoundTripTime === "number" &&
      Number.isFinite(pair.currentRoundTripTime)
        ? Math.max(0, pair.currentRoundTripTime * 1000)
        : null,
  };
}
