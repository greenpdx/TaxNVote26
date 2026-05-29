import { useState } from "react";

const COLORS = {
  bg: "#0d1117",
  surface: "#161b22",
  border: "#30363d",
  accent: "#f97316",
  accentDim: "#f9731640",
  rust: "#dc2626",
  rustDim: "#dc262630",
  js: "#3b82f6",
  jsDim: "#3b82f630",
  transfer: "#a855f7",
  transferDim: "#a855f730",
  text: "#e6edf3",
  textDim: "#8b949e",
  green: "#22c55e",
  greenDim: "#22c55e30",
};

function Arrow({ color = COLORS.accent, label, direction = "down" }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", alignItems: "center", margin: "4px 0" }}>
      {label && (
        <span style={{
          fontSize: 11,
          color: color,
          fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
          marginBottom: 2,
          textAlign: "center",
          lineHeight: 1.2,
        }}>{label}</span>
      )}
      <svg width="24" height="20" viewBox="0 0 24 20">
        {direction === "down" ? (
          <path d="M12 2 L12 14 M6 10 L12 16 L18 10" stroke={color} strokeWidth="2.5" fill="none" strokeLinecap="round" strokeLinejoin="round"/>
        ) : (
          <path d="M12 18 L12 6 M6 10 L12 4 L18 10" stroke={color} strokeWidth="2.5" fill="none" strokeLinecap="round" strokeLinejoin="round"/>
        )}
      </svg>
    </div>
  );
}

function Section({ title, color, colorDim, children, icon, defaultOpen = false }) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div
      onClick={() => setOpen(!open)}
      style={{
        background: colorDim,
        border: `2px solid ${color}`,
        borderRadius: 12,
        padding: "14px 16px",
        marginBottom: 10,
        cursor: "pointer",
        transition: "all 0.2s",
      }}
    >
      <div style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
      }}>
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <span style={{ fontSize: 22 }}>{icon}</span>
          <span style={{
            fontSize: 17,
            fontWeight: 700,
            color: color,
            fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
            letterSpacing: "-0.02em",
          }}>{title}</span>
        </div>
        <span style={{
          fontSize: 18,
          color: color,
          transform: open ? "rotate(180deg)" : "rotate(0deg)",
          transition: "transform 0.2s",
        }}>▾</span>
      </div>
      {open && (
        <div style={{ marginTop: 12, paddingTop: 10, borderTop: `1px solid ${color}40` }}>
          {children}
        </div>
      )}
    </div>
  );
}

function DataItem({ label, type, note }) {
  return (
    <div style={{
      display: "flex",
      alignItems: "baseline",
      gap: 8,
      marginBottom: 6,
      flexWrap: "wrap",
    }}>
      <span style={{
        color: COLORS.text,
        fontSize: 14,
        fontWeight: 600,
        fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
      }}>{label}</span>
      <span style={{
        color: COLORS.accent,
        fontSize: 12,
        fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
      }}>{type}</span>
      {note && (
        <span style={{
          color: COLORS.textDim,
          fontSize: 12,
        }}>— {note}</span>
      )}
    </div>
  );
}

function FlowStep({ num, text, color = COLORS.accent }) {
  return (
    <div style={{ display: "flex", alignItems: "flex-start", gap: 10, marginBottom: 8 }}>
      <span style={{
        background: color,
        color: COLORS.bg,
        width: 22,
        height: 22,
        borderRadius: "50%",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        fontSize: 12,
        fontWeight: 800,
        flexShrink: 0,
        marginTop: 1,
      }}>{num}</span>
      <span style={{
        color: COLORS.text,
        fontSize: 14,
        lineHeight: 1.4,
      }}>{text}</span>
    </div>
  );
}

export default function TNVArchitecture() {
  return (
    <div style={{
      background: COLORS.bg,
      minHeight: "100vh",
      padding: "20px 14px",
      fontFamily: "-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
      maxWidth: 480,
      margin: "0 auto",
    }}>
      {/* Header */}
      <div style={{ marginBottom: 24, textAlign: "center" }}>
        <h1 style={{
          color: COLORS.accent,
          fontSize: 22,
          fontWeight: 800,
          fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
          margin: 0,
          letterSpacing: "-0.03em",
        }}>TNV Budget Tree</h1>
        <p style={{
          color: COLORS.textDim,
          fontSize: 13,
          margin: "6px 0 0 0",
        }}>WASM Architecture · Tap sections to expand</p>
      </div>

      {/* JS Layer */}
      <Section title="JS / Vue Layer" color={COLORS.js} colorDim={COLORS.jsDim} icon="🖥" defaultOpen={true}>
        <p style={{ color: COLORS.textDim, fontSize: 13, margin: "0 0 10px 0" }}>
          Display only. Holds NO budget data. Calls into Rust for everything.
        </p>
        <DataItem label="init(json)" type="→ once" note="sends budget JSON at page load" />
        <DataItem label="adjust(id, val)" type="→ per drag" note="slider sends id + f64" />
        <DataItem label="lock(id)" type="→ on click" note="toggle node lock" />
        <DataItem label="get_value(id)" type="→ query" note="returns single f64" />
        <DataItem label="reset(id)" type="→ action" note="restore defaults" />
        <DataItem label="get_all_values()" type="→ once" note="initial display load" />
      </Section>

      <Arrow color={COLORS.transfer} label="id + f64 in" />

      {/* WASM Boundary */}
      <Section title="WASM Boundary" color={COLORS.transfer} colorDim={COLORS.transferDim} icon="⚡">
        <p style={{ color: COLORS.textDim, fontSize: 13, margin: "0 0 10px 0" }}>
          Minimal data crosses. No serialization of the tree.
        </p>
        <div style={{
          background: COLORS.bg,
          borderRadius: 8,
          padding: "10px 12px",
          marginBottom: 8,
        }}>
          <p style={{ color: COLORS.green, fontSize: 13, fontWeight: 700, margin: "0 0 6px 0" }}>IN (JS → Rust):</p>
          <DataItem label="JSON string" type="once" note="build tree" />
          <DataItem label="id: &str" type="per call" />
          <DataItem label="val: f64" type="per call" />
        </div>
        <div style={{
          background: COLORS.bg,
          borderRadius: 8,
          padding: "10px 12px",
        }}>
          <p style={{ color: COLORS.accent, fontSize: 13, fontWeight: 700, margin: "0 0 6px 0" }}>OUT (Rust → JS):</p>
          <DataItem label="changeset" type="[(idx, val)]" note="~20 entries per drag" />
          <DataItem label="f64 slice" type="[f64; N]" note="all values, once at init" />
          <DataItem label="single f64" type="" note="for queries" />
        </div>
      </Section>

      <Arrow color={COLORS.transfer} label="changeset out" direction="up" />

      {/* Rust Core */}
      <Section title="Rust / WASM Core" color={COLORS.rust} colorDim={COLORS.rustDim} icon="🦀" defaultOpen={true}>
        <p style={{ color: COLORS.textDim, fontSize: 13, margin: "0 0 10px 0" }}>
          Owns ALL budget data. Tree is opaque to JS.
        </p>

        {/* BudgetTree */}
        <div style={{
          background: COLORS.bg,
          borderRadius: 8,
          padding: "10px 12px",
          marginBottom: 8,
          border: `1px solid ${COLORS.rust}60`,
        }}>
          <p style={{
            color: COLORS.rust,
            fontSize: 14,
            fontWeight: 700,
            fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
            margin: "0 0 8px 0",
          }}>BudgetTree</p>
          <DataItem label="nodes" type="Vec&lt;BudgetNode&gt;" note="flat arena" />
          <DataItem label="id_map" type="HashMap&lt;String,usize&gt;" note="O(1) lookup" />
          <DataItem label="config" type="BudgetConfig" note="tuning knobs" />
        </div>

        {/* BudgetNode */}
        <div style={{
          background: COLORS.bg,
          borderRadius: 8,
          padding: "10px 12px",
          marginBottom: 8,
          border: `1px solid ${COLORS.rust}60`,
        }}>
          <p style={{
            color: COLORS.rust,
            fontSize: 14,
            fontWeight: 700,
            fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
            margin: "0 0 8px 0",
          }}>BudgetNode</p>
          <DataItem label="idx" type="usize" note="arena index, stable" />
          <DataItem label="id" type="String" note='"defense"' />
          <DataItem label="name" type="String" note='"Department of Defense"' />
          <DataItem label="value" type="f64" note="ONLY mutable field ✏️" />
          <DataItem label="default_value" type="f64" note="for reset & min-bound" />
          <DataItem label="locked" type="bool" note="user toggle 🔒" />
          <DataItem label="parent" type="usize" note="usize::MAX = root" />
          <DataItem label="children" type="Vec&lt;usize&gt;" note="empty = leaf" />
        </div>

        {/* Change */}
        <div style={{
          background: COLORS.bg,
          borderRadius: 8,
          padding: "10px 12px",
          border: `1px solid ${COLORS.accent}60`,
        }}>
          <p style={{
            color: COLORS.accent,
            fontSize: 14,
            fontWeight: 700,
            fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
            margin: "0 0 8px 0",
          }}>Change (transfer unit)</p>
          <DataItem label="idx" type="usize" note="which node" />
          <DataItem label="new_val" type="f64" note="its new value" />
          <p style={{ color: COLORS.textDim, fontSize: 12, margin: "8px 0 0 0" }}>
            ~16 bytes per changed node. Typical slider drag: ~20 nodes = ~320 bytes.
          </p>
        </div>
      </Section>

      {/* Adjust Flow */}
      <div style={{ marginTop: 16 }}>
        <Section title="Adjust Flow" color={COLORS.green} colorDim={COLORS.greenDim} icon="🔄">
          <FlowStep num="1" text='JS calls adjust("defense", 15.0)' color={COLORS.js} />
          <FlowStep num="2" text="Rust looks up node by id → O(1) HashMap" color={COLORS.rust} />
          <FlowStep num="3" text="Compute delta, check locked, check bounds" color={COLORS.rust} />
          <FlowStep num="4" text="Redistribute delta to unlocked siblings proportionally" color={COLORS.rust} />
          <FlowStep num="5" text="Each changed sibling rescales its children recursively" color={COLORS.rust} />
          <FlowStep num="6" text="Enforce exact sum (fix rounding drift)" color={COLORS.rust} />
          <FlowStep num="7" text="Collect all (idx, new_val) into changeset" color={COLORS.accent} />
          <FlowStep num="8" text="Return changeset to JS — JS patches only those DOM nodes" color={COLORS.js} />
          <div style={{
            background: COLORS.bg,
            borderRadius: 8,
            padding: "10px 12px",
            marginTop: 8,
          }}>
            <p style={{ color: COLORS.green, fontSize: 13, fontWeight: 700, margin: 0 }}>
              Target: &lt; 16ms per adjustment (60 FPS)
            </p>
            <p style={{ color: COLORS.textDim, fontSize: 12, margin: "4px 0 0 0" }}>
              1,000+ nodes · &lt; 500KB WASM binary
            </p>
          </div>
        </Section>
      </div>
    </div>
  );
}
