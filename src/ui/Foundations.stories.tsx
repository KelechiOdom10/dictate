import palettes from "./palettes.generated.json";
import contrast from "./contrast.generated.json";
import { Toast } from "./Content";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { tokens } from "./tokens";

function Foundations() {
  const type = [
    "fontXs",
    "fontSm",
    "fontBody",
    "fontHeading",
    "fontTitle",
  ] satisfies (keyof typeof tokens)[];
  const spacing = [
    "space0",
    "spaceHalf",
    "space1",
    "space2",
    "space3",
    "space4",
    "space5",
    "space6",
    "space8",
    "space10",
    "space16",
  ] satisfies (keyof typeof tokens)[];
  const radii = [
    "radiusControl",
    "radiusPanel",
    "radiusPill",
  ] satisfies (keyof typeof tokens)[];
  return (
    <main style={{ padding: tokens.space8 }}>
      <h1>Foundations</h1>
      <p className="ui-preview">
        Paper and Ember · warm brown neutrals at 75° · ink actions. Ember means
        recording, live or on.
      </p>
      <section className="foundation-section">
        <h2>Palettes · Ember and Paper</h2>
        <div className="palette-grid">
          {palettes.map(({ theme, mode, colors }) => (
            <section
              key={`${theme}-${mode}`}
              style={{
                background: colors.canvas,
                color: colors.text,
                padding: tokens.space4,
                borderRadius: tokens.radiusPanel,
              }}
            >
              <h3>
                {theme} · {mode}
              </h3>
              <div className="foundation-grid">
                {(
                  [
                    "canvas",
                    "sidebar",
                    "surface",
                    "control",
                    "control-hover",
                    "control-pressed",
                    "selected",
                    "primary",
                    "live",
                    "toggle-off",
                  ] as const
                ).map((name) => (
                  <div key={name}>
                    <div
                      className="foundation-swatch"
                      style={{ background: colors[name] }}
                    />
                    <code>{name}</code>
                  </div>
                ))}
              </div>
              <div className="story-stack">
                {(
                  [
                    ["success-text", "success", "Success · Text copied"],
                    [
                      "warning",
                      "warning-surface",
                      "Warning · Choose another microphone",
                    ],
                    ["danger", "danger-surface", "Danger · Download failed"],
                    ["info", "info-surface", "Info · Audio stays on this Mac"],
                  ] as const
                ).map(([fg, bg, label]) => (
                  <p
                    key={fg}
                    style={{
                      color: colors[fg],
                      background: colors[bg],
                      padding: tokens.space3,
                      borderRadius: tokens.radiusControl,
                    }}
                  >
                    {label}
                  </p>
                ))}
              </div>
              <p style={{ marginTop: tokens.space4 }}>
                Live{" "}
                <span
                  style={{
                    display: "inline-block",
                    width: 12,
                    height: 12,
                    borderRadius: 99,
                    background: colors.live,
                  }}
                />{" "}
                · Danger{" "}
                <span style={{ color: colors.danger }}>Download failed</span>
              </p>
            </section>
          ))}
        </div>
        <p className="ui-preview">
          Danger is red at 22°, 18° from ember at 40° and set apart by lightness
          too. Paper's record dot uses a brighter red from the same hue. Labels
          and icons distinguish errors from recording; compare them side by side
          above.
        </p>
      </section>
      <section className="foundation-section">
        <h2>Status surfaces</h2>
        <div className="story-stack">
          <Toast>Copied to clipboard.</Toast>
          <Toast tone="warning">
            Microphone disconnected. Choose another input.
          </Toast>
          <Toast tone="danger">Download failed. Try again.</Toast>
          <Toast tone="info">Your audio stays on this Mac.</Toast>
        </div>
      </section>
      <section className="foundation-section">
        <h2>Measured contrast · WCAG AA</h2>
        <p className="ui-preview">
          Chromium sRGB pixels · 4.5:1 for text, 3:1 for focus. Regenerate with
          node scripts/check-contrast.mjs.
        </p>
        <div className="contrast-table">
          <table>
            <thead>
              <tr>
                <th>Text / surface</th>
                {palettes.map(({ theme, mode }) => (
                  <th key={`${theme}-${mode}`}>
                    {theme} · {mode}
                  </th>
                ))}
                <th>Minimum</th>
              </tr>
            </thead>
            <tbody>
              {contrast
                .filter(
                  (pair) => pair.theme === "ember" && pair.mode === "light",
                )
                .map((pair) => (
                  <tr key={`${pair.foreground}-${pair.background}`}>
                    <th scope="row">
                      {pair.foreground} / {pair.background}
                    </th>
                    {palettes.map(({ theme, mode }) => (
                      <td key={`${theme}-${mode}`}>
                        {contrast
                          .find(
                            (p) =>
                              p.theme === theme &&
                              p.mode === mode &&
                              p.foreground === pair.foreground &&
                              p.background === pair.background,
                          )
                          ?.ratio.toFixed(2)}
                        :1
                      </td>
                    ))}
                    <td>{pair.minimum}:1</td>
                  </tr>
                ))}
            </tbody>
          </table>
        </div>
      </section>
      <section className="foundation-section">
        <h2>Typography · Instrument Sans</h2>
        <p className="ui-preview">
          22px title · 18px content heading · 13px row label and md control ·
          12px helper and section overline. Regular 400, medium 500, semibold
          600. Sentence case needs no added tracking.
        </p>
        <div className="story-stack">
          {type.map((name) => (
            <p key={name} style={{ fontSize: tokens[name] }}>
              {name} · Your voice, in your own words.
            </p>
          ))}
        </div>
      </section>
      <section className="foundation-section">
        <h2>Spacing</h2>
        <div className="story-stack">
          {spacing.map((name) => (
            <div key={name} className="story-row">
              <code>{name}</code>
              <div
                style={{
                  width: tokens[name],
                  height: tokens.space2,
                  background: tokens.accent,
                }}
              />
            </div>
          ))}
        </div>
      </section>
      <section className="foundation-section">
        <h2>Radii</h2>
        <div className="story-row">
          {radii.map((name) => (
            <div
              key={name}
              style={{
                borderRadius: tokens[name],
                background: tokens.surface,
                padding: tokens.space5,
              }}
            >
              {name}
            </div>
          ))}
        </div>
      </section>
      <section className="foundation-section">
        <h2>Motion</h2>
        <p>
          Fast: 120 ms · Normal: 200 ms. Reduced motion removes transitions and
          the spinner animation.
        </p>
      </section>
    </main>
  );
}
const meta = {
  title: "Foundations",
  component: Foundations,
  parameters: { layout: "fullscreen" },
} satisfies Meta<typeof Foundations>;
export default meta;
type Story = StoryObj<typeof meta>;
export const Palettes: Story = {};
