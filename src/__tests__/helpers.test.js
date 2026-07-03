import { describe, it, expect } from "vitest";

// Pure function tests — no DOM, no Tauri API needed

// ── esc ──

function esc(s) {
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

describe("esc", () => {
  it("escapes & < > \"", () => {
    expect(esc("<script>&\"")).toBe("&lt;script&gt;&amp;&quot;");
  });

  it("returns empty string for empty input", () => {
    expect(esc("")).toBe("");
  });

  it("handles numbers", () => {
    expect(esc(42)).toBe("42");
  });

  it("handles null", () => {
    expect(esc(null)).toBe("null");
  });

  it("handles undefined", () => {
    expect(esc(undefined)).toBe("undefined");
  });

  it("passes through safe strings unchanged", () => {
    expect(esc("hello world")).toBe("hello world");
  });
});

// ── fmtDuration ──

function fmtDuration(s) {
  s = Math.floor(s);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:${String(sec).padStart(2, "0")}`;
}

describe("fmtDuration", () => {
  it("formats zero", () => {
    expect(fmtDuration(0)).toBe("00:00:00");
  });

  it("formats seconds", () => {
    expect(fmtDuration(59)).toBe("00:00:59");
  });

  it("formats minutes", () => {
    expect(fmtDuration(60)).toBe("00:01:00");
    expect(fmtDuration(3660)).toBe("01:01:00");
  });

  it("formats hours", () => {
    expect(fmtDuration(3600)).toBe("01:00:00");
    expect(fmtDuration(3661)).toBe("01:01:01");
    expect(fmtDuration(86399)).toBe("23:59:59");
  });

  it("floors fractional seconds", () => {
    expect(fmtDuration(3661.7)).toBe("01:01:01");
  });
});

// ── formatError ──

function formatError(e) {
  return String(e?.message ?? e);
}

describe("formatError", () => {
  it("formats Error objects", () => {
    expect(formatError(new Error("boom"))).toBe("boom");
  });

  it("passes through strings", () => {
    expect(formatError("simple error")).toBe("simple error");
  });

  it("handles null", () => {
    expect(formatError(null)).toBe("null");
  });

  it("handles undefined", () => {
    expect(formatError(undefined)).toBe("undefined");
  });

  it("handles objects without message", () => {
    expect(formatError({ code: 500 })).toBe("[object Object]");
  });
});

// ── osDefaultKeypath ──

function osDefaultKeypath() {
  return "~/pot-o-miner-cli/miner.json";
}

describe("osDefaultKeypath", () => {
  it("returns expected path", () => {
    expect(osDefaultKeypath()).toBe("~/pot-o-miner-cli/miner.json");
  });
});

// ── TOKEN_TYPES ──

const TOKEN_TYPES = ["tribechain", "pttc", "nmtc", "stomp", "aum", "ai3"];

describe("TOKEN_TYPES", () => {
  it("has 6 tokens", () => {
    expect(TOKEN_TYPES).toHaveLength(6);
  });

  it("includes tribechain", () => {
    expect(TOKEN_TYPES).toContain("tribechain");
  });
});

// ── Config helpers (sel, seld, selmode, selpool) ──

function makeSelHelpers(initialConfig) {
  const config = { ...initialConfig };
  return {
    sel: (v) => (config.operation === v ? "selected" : ""),
    seld: (v) => (config.device_type === v ? "selected" : ""),
    selmode: (v) => (config.peer_network_mode === v ? "selected" : ""),
    selpool: (v) => (config.pool_strategy === v ? "selected" : ""),
  };
}

describe("config helpers", () => {
  it("sel returns selected on match", () => {
    const { sel } = makeSelHelpers({ operation: "relu" });
    expect(sel("relu")).toBe("selected");
    expect(sel("tanh")).toBe("");
  });

  it("seld returns selected on match", () => {
    const { seld } = makeSelHelpers({ device_type: "gpu" });
    expect(seld("gpu")).toBe("selected");
    expect(seld("cpu")).toBe("");
  });

  it("selmode returns selected on match", () => {
    const { selmode } = makeSelHelpers({ peer_network_mode: "vpn_mesh" });
    expect(selmode("vpn_mesh")).toBe("selected");
    expect(selmode("local_only")).toBe("");
  });

  it("selpool returns selected on match", () => {
    const { selpool } = makeSelHelpers({ pool_strategy: "pplns" });
    expect(selpool("pplns")).toBe("selected");
    expect(selpool("solo")).toBe("");
  });
});
