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
      <p className="ui-preview">Quiet surfaces. Clear words. Room to think.</p>
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
        <h2>Typography</h2>
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
