import contrast from "./contrast.generated.json";
import { Toast } from "./Content";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { tokens } from "./tokens";

function Foundations() {
  const colours = [
    "neutral0",
    "neutral50",
    "neutral100",
    "neutral200",
    "neutral300",
    "neutral400",
    "neutral500",
    "neutral600",
    "neutral700",
    "neutral800",
    "neutral900",
    "neutral950",
    "fg",
    "bg",
    "surface",
    "border",
    "accent",
    "success",
    "warning",
    "danger",
  ] satisfies (keyof typeof tokens)[];
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
        Cool slate neutrals at 255° · monochrome actions · status colour only
        when it carries meaning.
      </p>
      <section className="foundation-section">
        <h2>Colour</h2>
        <div className="foundation-grid">
          {colours.map((name) => (
            <div key={name}>
              <div
                className="foundation-swatch"
                style={{ background: tokens[name] }}
              />
              <code>{name}</code>
            </div>
          ))}
        </div>
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
                <th>Light</th>
                <th>Dark</th>
                <th>Minimum</th>
              </tr>
            </thead>
            <tbody>
              {contrast
                .filter((pair) => pair.theme === "light")
                .map((pair) => (
                  <tr key={`${pair.foreground}-${pair.background}`}>
                    <th scope="row">
                      {pair.foreground} / {pair.background}
                    </th>
                    <td>{pair.ratio.toFixed(2)}:1</td>
                    <td>
                      {contrast
                        .find(
                          (dark) =>
                            dark.theme === "dark" &&
                            dark.foreground === pair.foreground &&
                            dark.background === pair.background,
                        )
                        ?.ratio.toFixed(2)}
                      :1
                    </td>
                    <td>{pair.minimum}:1</td>
                  </tr>
                ))}
            </tbody>
          </table>
        </div>
      </section>
      <section className="foundation-section">
        <h2>Typography · system SF Pro</h2>
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
export const Light: Story = { globals: { theme: "light" } };
export const Dark: Story = { globals: { theme: "dark" } };
